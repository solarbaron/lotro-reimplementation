// SPDX-License-Identifier: GPL-3.0-or-later
//! P4 exit criterion 2 (decision 0024 P4, decision 0035 D1): tells and named chat channels
//! on Tessera's own test server, from the chat window's dispatcher path. An in-process
//! `tessera-server` on loopback, two clients (the client's own `net::World`, plugin host with
//! the command dispatcher, and game-UI host); every line goes in as the chat window's entry
//! hands it over (`EntryLine` with the window's channel mode, decision 0020 item 1) and runs
//! through `UiHost::dispatch_entry_lines`, exactly as a typed line does:
//!
//! - Aldith tells Bram (`/tell Bram …`); Bram sees the incoming tell line and replies
//!   (`/reply …`, the reply target of `chat-window.md` §15.10 CT3); Aldith tells again in the
//!   `Tell` mode (plain text with the mode's word and target, §7.2 step 5);
//! - both join a named channel (`/join Lore`, `/join lore`: one channel, the server's
//!   spelling), Aldith speaks on it (`/ch1 …`), Bram hears it in slot 1's line form; Bram
//!   leaves (`/part Lore`), Aldith speaks again and Bram no longer hears it (scenario-feed.md
//!   §14.13 S9).
//!
//! Needs no game install and no GPU. Skips itself (reporting "passed", and printing
//! `SKIPPED`) only where loopback UDP is unavailable.

use std::time::{Duration, Instant};

use tessera_client::net::{ChatLine, RemoteSettings, World};
use tessera_client::plugins::Plugins;
use tessera_client::ui_host::{UiHost, UiOptions};
use tessera_net::remote::Password;
use tessera_net::{ChatChannel, SessionState};
use tessera_protocol::types::{Appearance, Body};
use tessera_server::accounts::{self, HashParams};
use tessera_server::characters::{CharacterFile, CharacterRecord, StoredAppearance};
use tessera_server::{Config, Content, DataDir, Server, ServerHandle, ServerOptions};
use tessera_ui::game::chat::EntryLine;
use tessera_ui::game::chat::entry::{SAY, TELL};
use tessera_ui::game::chat::social::slot_mask;

const PASSWORD: &str = "correct horse battery";
/// Longest wait for one step.
const STEP: Duration = Duration::from_secs(20);
/// Client frame interval.
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

/// A server with accounts `alice` (character Aldith) and `bob` (character Bram), or `None`
/// when loopback UDP is unavailable.
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

/// One client: its world session, its plugin host with the command dispatcher, and its
/// game-UI host (the chat window's entry lines go through it).
struct Client {
    world: World,
    plugins: Plugins,
    host: UiHost,
    /// Every chat line its session produced (P4 forms included).
    lines: Vec<ChatLine>,
    /// Every line the dispatcher posted (mask, text).
    local: Vec<(u64, String)>,
    _dir: tempfile::TempDir,
}

impl Client {
    fn new(server: &ServerHandle, account: &str, character: &str) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let settings = RemoteSettings {
            server: server.connect_string(),
            fingerprint: Some(server.fingerprint()),
            known_servers: None,
            accept_new_key: false,
            account: account.into(),
            password: Password::new(PASSWORD),
            character: Some(character.into()),
        };
        let world = World::remote(Instant::now(), &settings, None, None).expect("session");
        let mut plugins = Plugins::start(&dir.path().join("Plugins"), dir.path().join("data"), &[]);
        plugins.data_unavailable();
        let mut host = UiHost::without_install(&UiOptions::default());
        host.attach_plugins(&mut plugins, &[]);
        Self {
            world,
            plugins,
            host,
            lines: Vec::new(),
            local: Vec::new(),
            _dir: dir,
        }
    }

    /// One frame: the session, the plugin host, the lines.
    fn frame(&mut self, now: Instant) {
        self.world.update(now);
        self.plugins.pump();
        self.lines.extend(self.world.take_chat());
        self.local.extend(self.plugins.take_local_chat());
    }

    /// Submits a line as the chat window's entry does (`chat-window.md` §6.2 submit,
    /// §10 item 11) and runs it through the dispatcher with the world session.
    fn submit(&mut self, text: &str, mode: u32, word: &str, target: &str) {
        self.host.submit_entry_line(EntryLine {
            text: text.into(),
            mode,
            word: word.into(),
            target: target.into(),
        });
        let now = Instant::now();
        self.host
            .dispatch_entry_lines(&mut self.plugins, Some(&mut self.world), now);
        self.local.extend(self.plugins.take_local_chat());
    }

    fn type_line(&mut self, text: &str) {
        self.submit(text, SAY, "", "");
    }

    /// Whether a shown line (P4 form) with `mask` contains `needle`.
    fn has(&self, mask: u64, needle: &str) -> bool {
        self.lines.iter().any(|l| {
            l.shown
                .as_ref()
                .is_some_and(|(m, t)| *m == mask && t.contains(needle))
        })
    }

    fn named_lines(&self, needle: &str) -> usize {
        self.lines
            .iter()
            .filter(|l| matches!(l.channel, ChatChannel::Named { .. }) && l.text.contains(needle))
            .count()
    }
}

fn run_until(
    c: &mut [&mut Client; 2],
    what: &str,
    mut done: impl FnMut(&[&mut Client; 2]) -> bool,
) {
    let deadline = Instant::now() + STEP;
    loop {
        let now = Instant::now();
        for x in c.iter_mut() {
            x.frame(now);
        }
        if done(c) {
            return;
        }
        assert!(
            now < deadline,
            "timed out waiting for {what}: {:?} / {:?}; lines {:?} / {:?}; local {:?} / {:?}",
            c[0].world.net_status(),
            c[1].world.net_status(),
            c[0].lines,
            c[1].lines,
            c[0].local,
            c[1].local
        );
        std::thread::sleep(FRAME);
    }
}

#[test]
fn tells_and_named_channels_through_the_chat_windows_dispatcher() {
    let Some(server) = start_server() else {
        return;
    };
    let handle = server.handle.as_ref().expect("server");
    let mut alice = Client::new(handle, "alice", "Aldith");
    let mut bob = Client::new(handle, "bob", "Bram");
    let mut c = [&mut alice, &mut bob];
    run_until(&mut c, "both in the world", |c| {
        c.iter().all(|x| x.world.state() == SessionState::InWorld)
            && c.iter().all(|x| x.world.social().player().len() > 1)
    });
    for x in c.iter() {
        assert!(
            x.world.offers_user_chat(),
            "minor 2 with CHAT_CHANNELS (scenario-feed.md §9.8)"
        );
    }

    // 1. A tell, its incoming line and the reply target (CT1, CT3, CT4).
    c[0].type_line("/tell Bram Well met on the road.");
    run_until(&mut c, "Bram's incoming tell", |c| {
        c[1].has(0x20, "Well met on the road.")
    });
    assert_eq!(
        c[1].world.social().chat().targets().reply.as_deref(),
        Some("Aldith"),
        "the reply target is the sender (CT3)"
    );
    assert!(
        c[1].lines.iter().any(|l| l.im.is_some()),
        "the tell has its instant-message line (CT6)"
    );
    run_until(&mut c, "Aldith's own tell line", |c| {
        c[0].has(0x20, "Well met on the road.")
    });
    c[1].type_line("/reply Likewise, friend.");
    run_until(&mut c, "Aldith hears the reply", |c| {
        c[0].has(0x20, "Likewise, friend.")
    });
    // The `Tell` mode with a target: plain text runs `tell` with the target (§7.2 step 5).
    c[0].submit("One more thing.", TELL, "tell", "Bram");
    run_until(&mut c, "the Tell-mode line", |c| {
        c[1].has(0x20, "One more thing.")
    });

    // 2. A named channel: join (one channel, the server's spelling), speak, leave.
    c[0].type_line("/join Lore");
    run_until(&mut c, "Aldith joined", |c| {
        c[0].world.social().chat().slot_of("Lore") == Some(1)
    });
    c[1].type_line("/join lore");
    run_until(&mut c, "Bram joined", |c| {
        c[1].world.social().chat().slot_of("Lore") == Some(1)
    });
    assert_eq!(
        c[1].world.social().chat().slots()[0].as_deref(),
        Some("Lore"),
        "the server's spelling (scenario-feed.md §14.13 S3)"
    );
    c[0].type_line("/ch1 Who has the old maps?");
    run_until(&mut c, "Bram hears the channel line", |c| {
        c[1].has(slot_mask(1), "Who has the old maps?")
    });
    assert!(
        c[0].has(slot_mask(1), "Who has the old maps?"),
        "Aldith gets her own echo in the slot's line form (CT1)"
    );
    c[1].type_line("/part Lore");
    run_until(&mut c, "Bram left", |c| {
        c[1].world.social().chat().slot_of("Lore").is_none()
    });
    let heard = c[1].named_lines("Anyone still here?");
    c[0].type_line("/ch1 Anyone still here?");
    run_until(&mut c, "Aldith's echo after Bram left", |c| {
        c[0].has(slot_mask(1), "Anyone still here?")
    });
    // A few more frames: nothing reaches Bram.
    for _ in 0..30 {
        let now = Instant::now();
        for x in c.iter_mut() {
            x.frame(now);
        }
        std::thread::sleep(FRAME);
    }
    assert_eq!(
        c[1].named_lines("Anyone still here?"),
        heard,
        "a line on a channel reaches only its members"
    );
    // `/channels` lists Aldith's one slot; no dispatcher line was an error.
    c[0].type_line("/channels");
    assert!(
        c[0].local.iter().any(|(_, t)| t.contains("Lore")),
        "{:?}",
        c[0].local
    );
    for x in c.iter() {
        assert!(
            x.local.iter().all(|(m, _)| m & 1 == 0),
            "no refusal: {:?}",
            x.local
        );
    }
}
