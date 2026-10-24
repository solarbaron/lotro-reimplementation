// SPDX-License-Identifier: GPL-3.0-or-later
//! The writer, the accounts file, the key backups, the server lock and in-process start-up
//! checks (`test-server.md` §2.2, §2.3, §3.4, §3.7, §8.1, §10.1).

use std::sync::Arc;
use std::time::Duration;

use tessera_server::accounts::{self, Account, AccountsFile};
use tessera_server::datadir::ServerLock;
use tessera_server::error::ServerError;
use tessera_server::shared::{SimInput, Stats, WriteJob, WriterMsg};
use tessera_server::workers::Writer;
use tessera_server::{Config, Content, DataDir, Server, ServerOptions};

fn config() -> Config {
    let mut c = Config::default();
    c.server.listen = vec!["127.0.0.1:0".parse().unwrap()];
    c.use_hash_floor();
    c.accounts.hash_workers = 1;
    c.log.stats_interval_s = 0;
    c
}

/// Finding: the writer reported `WriteDone` with a blocking send into the simulation's
/// input channel; with that channel full it never reached a barrier, so a shutdown stalled
/// for 60 s and reported a failure.
#[test]
fn writer_answers_barriers_while_the_simulation_channel_is_full() {
    let tmp = tempfile::tempdir().unwrap();
    let (tx, rx) = std::sync::mpsc::sync_channel::<SimInput>(1);
    tx.send(SimInput::Closed(0)).unwrap();
    let writer = Writer::start(tx.clone(), None, Arc::new(Stats::default())).unwrap();
    let w = writer.sender();
    w.send(WriterMsg::Write(WriteJob {
        path: tmp.path().join("a.toml"),
        contents: b"format = 1\n".to_vec(),
        mode: 0o600,
        token: Some(7),
    }))
    .unwrap();
    let (btx, brx) = std::sync::mpsc::channel();
    w.send(WriterMsg::Barrier(btx)).unwrap();
    assert_eq!(brx.recv_timeout(Duration::from_secs(5)), Ok(0));
    assert!(tmp.path().join("a.toml").exists());
    // The result is delivered once there is room.
    assert!(matches!(rx.recv().unwrap(), SimInput::Closed(0)));
    match rx.recv_timeout(Duration::from_secs(5)).unwrap() {
        SimInput::WriteDone { token, ok } => assert_eq!((token, ok), (7, true)),
        other => panic!("{other:?}"),
    }
    writer.stop();
}

/// Finding: registrations could grow `accounts.toml` past the 4 MiB every reader accepts,
/// after which the server refused to start.
#[test]
fn accounts_file_never_grows_past_what_readers_accept() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = DataDir::new(tmp.path().join("data"));
    dir.create().unwrap();
    let mut file = AccountsFile::default();
    let hash = format!(
        "$argon2id$v=19$m=19456,t=2,p=1${}${}",
        "A".repeat(22),
        "B".repeat(43)
    );
    for i in 0..30_000 {
        let name = format!("user{i}");
        file.accounts.insert(
            name.clone(),
            Account {
                name,
                hash: hash.clone(),
                created: "2026-10-07T00:00:00Z".into(),
                disabled: false,
                muted: false,
            },
        );
    }
    assert!(!accounts::fits(&file));
    assert!(matches!(
        accounts::write_accounts(&dir, &file),
        Err(ServerError::InvalidFile { .. })
    ));
    assert!(!dir.accounts_path().exists());
}

/// Finding: two forced keygens within one second overwrote the first backup.
#[cfg(unix)]
#[test]
fn forced_keygens_keep_every_backup() {
    use tessera_server::datadir;
    let tmp = tempfile::tempdir().unwrap();
    let dir = DataDir::new(tmp.path().join("data"));
    dir.create().unwrap();
    let (k0, _) = datadir::keygen(&dir, false).unwrap();
    let (k1, old0) = datadir::keygen(&dir, true).unwrap();
    let (_, old1) = datadir::keygen(&dir, true).unwrap();
    assert_eq!(old0, Some(k0.fingerprint()));
    assert_eq!(old1, Some(k1.fingerprint()));
    let backups = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("server.key.old-")
        })
        .count();
    assert_eq!(backups, 2);
}

/// Finding: `Content`'s fields are public, and a hand-built one without spawns made the
/// simulation thread panic on the first character creation.
#[test]
fn hand_built_content_and_config_are_checked() {
    let tmp = tempfile::tempdir().unwrap();
    let mut content = Content::default_world();
    content.spawns.clear();
    let r = Server::start(ServerOptions {
        data_dir: DataDir::new(tmp.path().join("a")),
        config: config(),
        content,
    });
    assert!(matches!(r, Err(ServerError::Usage(_))), "{r:?}");
    let mut cfg = config();
    cfg.world.instances = 0;
    let r = Server::start(ServerOptions {
        data_dir: DataDir::new(tmp.path().join("b")),
        config: cfg,
        content: Content::default_world(),
    });
    assert!(matches!(r, Err(ServerError::Usage(_))), "{r:?}");
}

/// Findings: `run` released the lock between start-up steps (now it passes its lock to
/// `Server::start_locked`), and `connect_string` gave an unspecified address.
#[test]
fn held_lock_and_dialable_connect_string() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = DataDir::new(tmp.path().join("data"));
    dir.create().unwrap();
    let lock = ServerLock::acquire(&dir, true).unwrap();
    let mut cfg = config();
    cfg.server.listen = vec!["0.0.0.0:0".parse().unwrap()];
    let handle = Server::start_locked(
        ServerOptions {
            data_dir: dir.clone(),
            config: cfg,
            content: Content::default_world(),
        },
        lock,
    )
    .unwrap();
    assert!(matches!(
        ServerLock::acquire(&dir, false),
        Err(ServerError::Locked { .. })
    ));
    let s = handle.connect_string();
    assert!(s.starts_with("tessera://127.0.0.1:"), "{s}");
    assert!(!s.contains("0.0.0.0"));
    let report = handle.shutdown(Duration::ZERO);
    assert_eq!(report.failures, 0);
    assert!(ServerLock::acquire(&dir, false).is_ok());
}
