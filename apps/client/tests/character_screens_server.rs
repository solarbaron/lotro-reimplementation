// SPDX-License-Identifier: GPL-3.0-or-later
//! A refused world entry on the character screens against Tessera's test server
//! (`tessera-protocol.md` §6.2.11 and Appendix A "Decision on `EnterWorldFailed`",
//! `character-screens.md` §7.3): an in-process `tessera-server` on loopback and a client on
//! the character screens (the client's `net::World` with manual entry, the screens pumping
//! its session as `crate::pre_world` does). The list also holds a name the server does not
//! know (a stale row, put in as a session's `CharacterInfo` would add it); entering it is
//! refused (`UNKNOWN_CHARACTER`): the screens go back to the selection with the reason as
//! an error line and Enter enabled, the session stays connected, and entering Bramble
//! afterwards succeeds.
//!
//! Needs no game install and no GPU. Skips itself (reporting "passed", and printing
//! `SKIPPED`) only where loopback UDP is unavailable; any failure to start the server is a
//! test failure.

#[path = "character_screens_common.rs"]
mod common;

use std::time::{Duration, Instant};

use tessera_client::net::{RemoteSettings, World};
use tessera_client::screens::{Action, CharacterScreens, ScreenOptions, Stage, Status};
use tessera_net::SessionState;
use tessera_net::domain::CharacterEvent;
use tessera_net::remote::Password;
use tessera_protocol::types::{Appearance, Body};
use tessera_server::accounts::{self, HashParams};
use tessera_server::characters::{CharacterFile, CharacterRecord, StoredAppearance};
use tessera_server::{Config, Content, DataDir, Server, ServerHandle, ServerOptions};

const PASSWORD: &str = "correct horse battery";
/// Longest wait for one step.
const STEP: Duration = Duration::from_secs(20);

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

/// A server with account `alice` (characters Aldith and Bramble), or `None` when loopback
/// UDP is unavailable.
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
        &[("alice", PASSWORD)],
    )
    .expect("accounts");
    std::fs::create_dir_all(data.characters_dir()).expect("characters dir");
    let record = |name: &str, species| CharacterRecord {
        name: name.into(),
        created: "2026-10-07T00:00:00Z".into(),
        instance: 0,
        position: None,
        appearance: StoredAppearance::Valid(Appearance::new(Body::Character { species, sex: 1 })),
        look: tessera_protocol::types::Look::none(),
    };
    let file = CharacterFile {
        account: "alice".into(),
        characters: vec![record("Aldith", 23), record("Bramble", 81)],
    };
    std::fs::write(data.character_path("alice"), file.to_toml()).expect("character file");
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

fn settings(server: &ServerHandle, character: &str) -> RemoteSettings {
    RemoteSettings {
        server: server.connect_string(),
        fingerprint: Some(server.fingerprint()),
        known_servers: None,
        accept_new_key: false,
        account: "alice".into(),
        password: Password::new(PASSWORD),
        character: Some(character.into()),
    }
}

/// The second client: its world session on manual entry, polled by the screens.
struct Screens {
    world: World,
    screens: CharacterScreens,
    events: Vec<tessera_net::Event>,
}

impl Screens {
    /// One frame as `crate::pre_world` runs it: the screens pump the session, the world
    /// session takes the polled events.
    fn frame(&mut self) {
        let now = Instant::now();
        self.events.clear();
        self.screens
            .pump(self.world.session_mut(), now, &mut self.events);
        self.world.apply_polled(now, &self.events);
    }

    fn until(&mut self, what: &str, done: impl Fn(&Self) -> bool) {
        let deadline = Instant::now() + STEP;
        while !done(self) {
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {what}: status {:?}, session {:?}, errors {:?}",
                self.screens.status(),
                self.world.state(),
                self.screens.model().error_texts()
            );
            self.frame();
            std::thread::sleep(common::FRAME);
        }
    }

    fn select(&mut self, name: &str) {
        let i = self
            .screens
            .model()
            .rows()
            .iter()
            .position(|r| r.name == name)
            .unwrap_or_else(|| panic!("{name} listed"));
        self.screens.act(Action::Select(i), Instant::now());
    }
}

#[test]
fn a_refused_entry_returns_to_the_selection_with_the_session_kept() {
    let Some(server) = start_server() else {
        return;
    };
    let handle = server.handle.as_ref().expect("server");
    let mut world =
        World::remote(Instant::now(), &settings(handle, "Aldith"), None, None).expect("session");
    world.set_manual_enter(true);
    world.set_external_poll(true);
    let screens = CharacterScreens::new(
        world.session_mut(),
        common::synthetic_data(),
        ScreenOptions::default(),
    );
    let mut s = Screens {
        world,
        screens,
        events: Vec::new(),
    };
    s.until("the character list", |s| {
        s.screens.model().rows().len() == 2
    });
    assert_eq!(s.screens.model().stage(), Stage::Select);
    // A row the server does not know (as a stale list entry would be).
    s.screens.model_mut().handle_event(
        &tessera_net::Event::Character(CharacterEvent::CharacterInfo {
            name: "Nobody".into(),
            body: None,
            class: None,
            level: None,
            look: None,
        }),
        Instant::now(),
    );
    s.select("Nobody");
    s.screens.act(Action::Enter, Instant::now());
    assert!(matches!(s.screens.status(), Status::Entering(_)));
    s.until("the refusal", |s| {
        !s.screens.model().error_texts().is_empty()
    });
    assert_eq!(
        s.screens.status(),
        &Status::Open,
        "back at the selection after EnterWorldFailed"
    );
    assert_eq!(s.screens.model().stage(), Stage::Select);
    let errors = s.screens.model().error_texts();
    assert!(
        errors.iter().any(|e| e.contains("UNKNOWN_CHARACTER")),
        "the reason is shown: {errors:?}"
    );
    assert!(
        s.screens.model().enter_button().is_enabled(),
        "Enter enabled again"
    );
    // The session is kept (`tessera-protocol.md` Appendix A).
    for _ in 0..10 {
        s.frame();
        std::thread::sleep(common::FRAME);
    }
    assert_eq!(
        s.world.state(),
        SessionState::Connected,
        "the session stays"
    );

    // Another character enters.
    s.select("Bramble");
    s.screens.act(Action::Enter, Instant::now());
    s.until(
        "Bramble in the world",
        |s| matches!(s.screens.status(), Status::Entered(n) if n == "Bramble"),
    );
    s.world.set_external_poll(false);
    s.world.shutdown();
}
