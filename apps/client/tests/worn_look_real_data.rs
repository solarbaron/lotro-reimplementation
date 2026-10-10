// SPDX-License-Identifier: GPL-3.0-or-later
//! Decision 0024 P1 exit criterion 3 on the user's install (read-only): "Equipping an item
//! from a scenario changes the worn look in the walker; observers on M7's server see it
//! through `AppearanceChanged`."
//!
//! 1. **Walker.** A headless scenario run with the walker (`scenario::screenshot`, the path
//!    of `--scenario … --screenshot`) gives the player a chest piece and a primary weapon
//!    and equips both; the avatar's figure request then holds both records with their slot
//!    keys (decision 0025 D3), its figure is rebuilt, and it differs from the figure before
//!    the equip.
//! 2. **Server.** An in-process `tessera-server` with two clients through the client's
//!    `net::World`: client A runs the same equipment steps as an overlay run
//!    (`scenario-feed.md` §12.5), client B observes. B receives `AppearanceChanged` for A's
//!    entity with the chest record in slot 3.
//!
//! Selectors use labels only (decision 0025, open item 1). Prints counts and identifiers
//! only. Skipped (reported as passed) unless `TESSERA_GAME_DIR` is set; part 1 also needs a
//! GPU adapter (headless), part 2 loopback UDP.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Parser as _;
use tessera_client::cli::Args;
use tessera_client::net::{RemoteSettings, World};
use tessera_client::scenario::{InstallChoice, prepare};
use tessera_client::walker::characters::FigureState;
use tessera_net::remote::Password;
use tessera_net::{EntityId, Event};
use tessera_protocol::types::{Appearance, Body};
use tessera_server::accounts::{self, HashParams};
use tessera_server::characters::{CharacterFile, CharacterRecord, StoredAppearance};
use tessera_server::{Config, Content, DataDir, Server, ServerHandle, ServerOptions};

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

/// The chest slot key and the primary-weapon key (`character-panels.md` §3.3 table).
const CHEST: u8 = 3;
const PRIMARY: u8 = 17;

/// The walker part: equipment steps on a player with a body.
const WALKER_SCENARIO: &str = r#"tessera-scenario 1
title "Worn look"
tick 100ms
player "Tamsin"
select $robes = item fits=@Chest equippable=yes n=1 seed=3
select $blades = item fits=@Weapon_Primary equippable=yes kind=weapon n=1 seed=3
item $robe $robes[0]
item $blade $blades[0]
at 0s give $robe
+0s give $blade
at 5s equip $robe slot=@Chest
+0s equip $blade slot=@Weapon_Primary
at 6s level player 2
"#;

/// Real time the observer gives the workers per driver poll while the player's figure
/// before the equip is still building. The driver's clock is virtual, so without this a
/// loaded machine can reach the equip (50 polls in) before the first figure is built; with
/// it the figure has up to 50 × this long.
const BUILD_WAIT_PER_POLL: Duration = Duration::from_millis(250);

/// The server part: the overlay subset of the same steps (no player declaration).
const OVERLAY_SCENARIO: &str = r#"tessera-scenario 1
title "Worn look on the server"
tick 50ms
select $robes = item fits=@Chest equippable=yes n=1 seed=3
item $robe $robes[0]
at 0.2s give $robe
at 0.5s equip $robe slot=@Chest
at 5s bag 10
"#;

/// The avatar's figure request (record and slot of each item) and its state.
type Seen = (Vec<(u32, Option<u8>)>, FigureState);

fn scenario_file(dir: &Path, name: &str, text: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, text).expect("write scenario");
    p
}

#[test]
fn a_scenario_equip_changes_the_walker_figure() {
    let Some(game) = game_dir() else { return };
    let dir = tempfile::tempdir().expect("tempdir");
    let file = scenario_file(dir.path(), "worn.scenario", WALKER_SCENARIO);
    let png = dir.path().join("worn.png");
    let mut args = Args::parse_from([
        "tessera-client",
        "--headless",
        "--settle-timeout",
        "60",
        "--size",
        "320x240",
        "--scenario",
        file.to_str().expect("utf-8 path"),
    ]);
    args.game_dir = Some(game.clone());
    args.screenshot = Some(png);
    args.data_dir = Some(dir.path().join("data"));
    let resolved =
        Arc::new(prepare(&args, &InstallChoice::Found(game)).expect("the scenario resolves"));
    let mut player: Option<EntityId> = None;
    // (figure request equipment, state) after each frame.
    let mut before: Option<Seen> = None;
    let mut last: Option<Seen> = None;
    let mut equipped_seen = false;
    let result = tessera_client::scenario::screenshot::run_observed(
        &args,
        Arc::clone(&resolved),
        None,
        &mut |walker, events| {
            for e in events {
                if let Event::WorldEntered { player: p, .. } = e {
                    player = Some(*p);
                }
            }
            let Some(p) = player else { return };
            let Some(f) = walker.characters().figure_of(p) else {
                return;
            };
            let items: Vec<(u32, Option<u8>)> = f
                .request
                .equipment
                .iter()
                .map(|e| (e.item.0, e.slot))
                .collect();
            if items.is_empty() && matches!(f.state, FigureState::Built { .. }) {
                before = Some((items.clone(), f.state));
            }
            if items.is_empty() && before.is_none() && f.state == FigureState::Pending {
                std::thread::sleep(BUILD_WAIT_PER_POLL);
            }
            equipped_seen |= !items.is_empty();
            last = Some((items, f.state));
        },
    );
    let code = match result {
        Ok(c) => c,
        Err(e) if format!("{e:#}").contains("headless renderer") => {
            eprintln!("SKIPPED: no GPU adapter ({e:#})");
            return;
        }
        Err(e) => panic!("walker run: {e:#}"),
    };
    assert_eq!(code, 0, "the scenario finishes");
    assert!(player.is_some(), "the player entered");
    let (before_items, before_state) = before.expect("a built figure before the equip");
    let (items, state) = last.expect("the avatar has a figure");
    eprintln!("before: {before_items:?} {before_state:?}; after: {items:?} {state:?}");
    assert!(equipped_seen);
    let slots: Vec<Option<u8>> = items.iter().map(|(_, s)| *s).collect();
    assert!(
        slots.contains(&Some(CHEST)),
        "the chest piece in slot 3: {items:?}"
    );
    assert!(
        slots.contains(&Some(PRIMARY)),
        "the weapon in slot 17: {items:?}"
    );
    let FigureState::Built { parts, held } = state else {
        panic!("the new figure is built: {state:?}");
    };
    let FigureState::Built {
        parts: parts_before,
        held: held_before,
    } = before_state
    else {
        panic!("the figure before the equip is built: {before_state:?}");
    };
    assert!(before_items.is_empty());
    assert_eq!(held_before, 0, "nothing held before the equip");
    assert!(held >= 1, "the weapon of slot 17 is a held part: {state:?}");
    assert_ne!(
        (parts, held),
        (parts_before, held_before),
        "the walker's figure changed"
    );
}

// ---------------------------------------------------------------------------------------
// Server part
// ---------------------------------------------------------------------------------------

const PASSWORD: &str = "correct horse battery";
const STEP: Duration = Duration::from_secs(30);
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
    assert!(
        config.server.appearance_change,
        "APPEARANCE_CHANGE is advertised"
    );
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
        created: "2026-10-10T00:00:00Z".into(),
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

#[test]
fn observers_on_the_test_server_see_the_equipped_item() {
    let Some(game) = game_dir() else { return };
    let Some(server) = start_server() else { return };
    let handle = server.handle.as_ref().expect("handle");
    let dir = tempfile::tempdir().expect("tempdir");
    let file = scenario_file(dir.path(), "overlay.scenario", OVERLAY_SCENARIO);
    let mut args = Args::parse_from([
        "tessera-client",
        "--headless",
        "--server",
        "127.0.0.1:9",
        "--account",
        "alice",
        "--scenario",
        file.to_str().expect("utf-8 path"),
    ]);
    args.game_dir = Some(game.clone());
    let resolved = Arc::new(
        prepare(&args, &InstallChoice::Found(game)).expect("the overlay scenario resolves"),
    );
    let now = Instant::now();
    let mut observer =
        World::remote(now, &settings(handle, "bob", "Bram"), None, None).expect("observer session");
    observer.capture_entity_events(true);
    let mut actor = World::overlay(
        now,
        &settings(handle, "alice", "Aldith"),
        Arc::clone(&resolved),
        None,
        None,
    )
    .expect("overlay session");
    let deadline = Instant::now() + STEP;
    let mut events = Vec::new();
    let mut seen: Option<Vec<(u32, Option<u8>)>> = None;
    let mut actor_view: Option<Vec<(u32, Option<u8>)>> = None;
    while seen.is_none() {
        let now = Instant::now();
        actor.update(now);
        observer.update(now);
        if let Some(a) = actor.player_state().appearance() {
            actor_view = Some(a.equipment.iter().map(|e| (e.item.0, e.slot)).collect());
        }
        let aldith = observer
            .entities()
            .iter()
            .find(|(_, v)| v.name == "Aldith")
            .map(|(id, _)| *id);
        events.clear();
        observer.drain_entity_events(&mut events, usize::MAX);
        for e in &events {
            if let Event::AppearanceChanged { id, appearance } = e
                && Some(*id) == aldith
                && appearance.equipment.iter().any(|q| q.slot == Some(CHEST))
            {
                seen = Some(
                    appearance
                        .equipment
                        .iter()
                        .map(|q| (q.item.0, q.slot))
                        .collect(),
                );
            }
        }
        assert!(
            now < deadline,
            "timed out: the observer saw no AppearanceChanged with the chest piece \
             (actor's own view {actor_view:?}, overlay sets sent {:?})",
            actor.overlay_backend().map(|o| o.appearance_sets_sent())
        );
        std::thread::sleep(FRAME);
    }
    let seen = seen.expect("seen");
    eprintln!("observer saw equipment {seen:?}");
    // The record the scenario equipped, in slot 3, as the overlay's own state holds it.
    let robe = actor
        .player_state()
        .inventory()
        .equipped()
        .find(|i| i.place.slot == u16::from(CHEST))
        .and_then(|i| match i.what {
            tessera_gamedata::inventory::ItemRef::Record(d) => Some(d.0),
            tessera_gamedata::inventory::ItemRef::Literal(_) => None,
        })
        .expect("the overlay equipped a record in slot 3");
    assert!(
        seen.contains(&(robe, Some(CHEST))),
        "the observer sees the equipped record {robe:#010x} in slot 3: {seen:?}"
    );
    actor.shutdown();
    observer.shutdown();
}
