// SPDX-License-Identifier: GPL-3.0-or-later
//! The command line, the data directory, the install guard and content checks
//! (`test-server.md` §2, §3, §5.6).

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use tessera_server::DataDir;
use tessera_server::datadir;

fn bin() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_tessera-server"));
    c.env_remove("RUST_LOG").env_remove("TESSERA_SERVER_DIR");
    c
}

fn run(args: &[&str], dir: &Path, stdin: Option<&str>) -> (i32, String, String) {
    let mut cmd = bin();
    cmd.arg("--data-dir").arg(dir).args(args);
    cmd.stdin(if stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    })
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    if let Some(s) = stdin {
        child.stdin.take().unwrap().write_all(s.as_bytes()).unwrap();
    }
    let out = child.wait_with_output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn fast_config(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(
        dir.join("server.toml"),
        "[server]\nlisten = [\"127.0.0.1:47123\"]\n[accounts]\nargon2_memory_kib = 19456\nargon2_iterations = 2\nargon2_lanes = 1\n",
    )
    .unwrap();
}

#[test]
fn help_states_the_operator_rules() {
    let out = bin().arg("--help").output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(text.contains("private"), "{text}");
    assert!(text.contains("legally obtained installation"));
    assert!(text.contains("money"));
}

#[test]
fn keygen_and_fingerprint() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("data");
    // No key yet: fingerprint fails (status 1) and creates nothing.
    let (code, _, _) = run(&["fingerprint"], &dir, None);
    assert_ne!(code, 0);
    assert!(!dir.join("server.key").exists());
    let (code, out, _) = run(&["keygen"], &dir, None);
    assert_eq!(code, 0);
    let fp = out.trim().to_owned();
    assert_eq!(fp.len(), 71);
    assert!(fp.starts_with("sha256:"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(dir.join("server.key"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        let dmode = std::fs::metadata(&dir).unwrap().permissions().mode();
        assert_eq!(dmode & 0o777, 0o700);
    }
    let (code, out, _) = run(&["fingerprint"], &dir, None);
    assert_eq!(code, 0);
    assert_eq!(out.trim(), fp);
    // A second keygen refuses without --force (status 2).
    let (code, _, _) = run(&["keygen"], &dir, None);
    assert_eq!(code, 2);
    let (code, out, err) = run(&["keygen", "--force"], &dir, None);
    assert_eq!(code, 0);
    assert!(out.contains(&fp), "{out}");
    assert!(err.contains("KEY_MISMATCH"));
    let backups = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("server.key.old-")
        })
        .count();
    assert_eq!(backups, 1);
    // Connect strings per configured port.
    fast_config(&dir);
    let (code, out, _) = run(&["fingerprint", "--connect", "2001:db8::1"], &dir, None);
    assert_eq!(code, 0);
    assert!(
        out.starts_with("tessera://[2001:db8::1]:47123?pin=sha256:"),
        "{out}"
    );
    // The key file never appears in any output.
    let key = std::fs::read_to_string(dir.join("server.key")).unwrap();
    let body: String = key.lines().nth(1).unwrap().to_owned();
    assert!(!out.contains(&body));
}

#[test]
fn key_permissions_are_checked() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = DataDir::new(tmp.path().join("data"));
    dir.create().unwrap();
    datadir::keygen(&dir, false).unwrap();
    assert!(datadir::load_key(&dir).is_ok());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.key_path(), std::fs::Permissions::from_mode(0o644)).unwrap();
        let e = datadir::load_key(&dir).unwrap_err();
        assert!(
            matches!(e, tessera_server::error::ServerError::KeyPermissions { .. }),
            "{e}"
        );
        assert!(e.to_string().contains("chmod 600"));
        assert_eq!(e.exit_status(), 1);
    }
    std::fs::write(
        dir.key_path(),
        "-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.key_path(), std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    assert_eq!(datadir::load_key(&dir).unwrap_err().exit_status(), 1);
}

#[test]
fn accounts_from_the_command_line() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("data");
    fast_config(&dir);
    // Without a terminal and without a source: status 2.
    let (code, _, _) = run(&["account", "add", "alice"], &dir, None);
    assert_eq!(code, 2);
    let (code, _, _) = run(
        &["account", "add", "Alice", "--password-stdin"],
        &dir,
        Some("first password\r\n"),
    );
    assert_eq!(code, 0);
    let (code, _, _) = run(
        &["account", "add", "alice", "--password-stdin"],
        &dir,
        Some("x y z w v u\n"),
    );
    assert_eq!(code, 2, "duplicate canonical name");
    let (code, out, _) = run(
        &["account", "add", "bob", "--generate", "--disabled"],
        &dir,
        None,
    );
    assert_eq!(code, 0);
    assert_eq!(out.trim().len(), 20);
    // Too short a password.
    let (code, _, _) = run(
        &["account", "add", "carol", "--password-stdin"],
        &dir,
        Some("short\n"),
    );
    assert_eq!(code, 2);
    let (code, out, _) = run(&["account", "list"], &dir, None);
    assert_eq!(code, 0);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2, "{out}");
    assert!(lines[0].starts_with("alice\t"));
    assert!(lines[1].starts_with("bob\t") && lines[1].contains("disabled"));
    assert!(!out.contains("argon2"), "hashes are never listed");
    // The stored hash verifies the password without the CRLF.
    let file = tessera_server::accounts::read_accounts(&DataDir::new(&dir)).unwrap();
    let h = tessera_server::accounts::PhcHash::parse(&file.accounts["alice"].hash).unwrap();
    assert!(h.verify(b"first password"));
    assert_eq!(h.params.memory_kib, 19_456);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(dir.join("accounts.toml"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    for (args, want) in [
        (vec!["account", "mute", "alice"], 0),
        (vec!["account", "enable", "bob"], 0),
        (vec!["account", "disable", "nobody"], 2),
    ] {
        assert_eq!(run(&args, &dir, None).0, want, "{args:?}");
    }
    let (code, _, _) = run(
        &["account", "passwd", "alice", "--password-stdin"],
        &dir,
        Some("second password\n"),
    );
    assert_eq!(code, 0);
    let file = tessera_server::accounts::read_accounts(&DataDir::new(&dir)).unwrap();
    assert!(file.accounts["alice"].muted);
    assert!(!file.accounts["bob"].disabled);
    let h = tessera_server::accounts::PhcHash::parse(&file.accounts["alice"].hash).unwrap();
    assert!(h.verify(b"second password"));
    // Remove needs --yes without a terminal; it removes the account.
    assert_eq!(run(&["account", "remove", "bob"], &dir, None).0, 2);
    assert_eq!(run(&["account", "remove", "bob", "--yes"], &dir, None).0, 0);
    let file = tessera_server::accounts::read_accounts(&DataDir::new(&dir)).unwrap();
    assert!(!file.accounts.contains_key("bob"));
    // Refused while the directory is locked by a running server (status 4).
    let lock = datadir::ServerLock::acquire(&DataDir::new(&dir), true).unwrap();
    let (code, _, err) = run(&["account", "remove", "alice", "--yes"], &dir, None);
    assert_eq!(code, 4, "{err}");
    assert!(err.contains(&std::process::id().to_string()), "{err}");
    drop(lock);
}

#[test]
fn check_content_reports_findings() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("data");
    std::fs::create_dir_all(&dir).unwrap();
    // The built-in default world and an empty configuration pass.
    let (code, out, _) = run(&["check-content"], &dir, None);
    assert_eq!(code, 0, "{out}");
    assert!(out.ends_with("0 findings in 2 files\n"), "{out}");
    // A file that breaks several rules (a negative fixture made of invented values and one
    // listed word for the word check).
    let bad = tmp.path().join("bad.toml");
    std::fs::write(
        &bad,
        "format = 1\n[world]\nid = \"Bad Id\"\ntitle = \"Gate of Moria\"\nmotd = \"\"\ndefault_spawn = \"x\"\nextra = 1\n[[spawn]]\nname = \"x\"\nregion = 99999\nlocation = [0.0, 0.0, 0.0]\n",
    )
    .unwrap();
    let (code, out, _) = run(&["check-content", bad.to_str().unwrap()], &dir, None);
    assert_eq!(code, 3);
    let lines: Vec<&str> = out.lines().collect();
    let rules: Vec<&str> = lines.iter().filter_map(|l| l.split(": ").nth(1)).collect();
    for r in ["SPDX", "TEXT", "WORD", "FORMAT", "RANGE"] {
        assert!(rules.contains(&r), "{r} missing in {out}");
    }
    let prefix = format!("{}:", bad.display());
    assert!(
        lines[..lines.len() - 1]
            .iter()
            .all(|l| l.starts_with(&prefix))
    );
    // Sorted by line.
    let nums: Vec<usize> = lines[..lines.len() - 1]
        .iter()
        .map(|l| {
            l[prefix.len()..]
                .split(':')
                .next()
                .unwrap()
                .parse()
                .unwrap()
        })
        .collect();
    assert!(nums.windows(2).all(|w| w[0] <= w[1]), "{nums:?}");
    // A configuration finding is reported with the default content.
    std::fs::write(
        dir.join("server.toml"),
        "[server]\nname = \"Mirkwood test\"\nbogus = true\n",
    )
    .unwrap();
    let (code, out, _) = run(&["check-content"], &dir, None);
    assert_eq!(code, 3);
    assert!(out.contains("WORD") && out.contains("CONFIG"), "{out}");
}

#[test]
fn install_guard_refuses_game_directories() {
    let tmp = tempfile::tempdir().unwrap();
    let game = tmp.path().join("game");
    std::fs::create_dir_all(game.join("sub")).unwrap();
    // An empty file with an archive-like name (synthetic; never a real archive).
    std::fs::write(game.join("client_test.dat"), b"").unwrap();
    let dir = game.join("sub").join("data");
    let (code, _, err) = run(&["keygen"], &dir, None);
    assert_eq!(code, 4, "{err}");
    assert!(!dir.exists(), "nothing is created inside an installation");
    let content = game.join("world.toml");
    std::fs::write(&content, tessera_server::content::DEFAULT_WORLD).unwrap();
    let (code, out, _) = run(
        &["check-content", content.to_str().unwrap()],
        tmp.path(),
        None,
    );
    assert_eq!(code, 3);
    assert!(out.contains("INSTALL-PATH"), "{out}");
    // A data directory holding a .dat file is refused too.
    let plain = tmp.path().join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    std::fs::write(plain.join("x.dat"), b"").unwrap();
    assert!(datadir::data_dir_guard_hit(&plain));
}

/// §3.1: `run` refuses a content file inside an installation with status 3 (`INSTALL-PATH`),
/// given on the command line or as `[server] content`; the file is not read.
#[test]
fn run_refuses_content_in_an_installation_with_status_3() {
    let tmp = tempfile::tempdir().unwrap();
    let game = tmp.path().join("game");
    std::fs::create_dir_all(&game).unwrap();
    // An empty file with an archive-like name (synthetic; never a real archive).
    std::fs::write(game.join("client_test.dat"), b"").unwrap();
    let content = game.join("world.toml");
    std::fs::write(&content, tessera_server::content::DEFAULT_WORLD).unwrap();
    let dir = tmp.path().join("data");
    fast_config(&dir);
    let (code, out, err) = run(&["run", "--content", content.to_str().unwrap()], &dir, None);
    assert_eq!(code, 3, "{out}{err}");
    assert!(
        out.contains("INSTALL-PATH") || err.contains("INSTALL-PATH"),
        "{out}{err}"
    );
    // The same file named in server.toml.
    let dir = tmp.path().join("data2");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("server.toml"),
        format!(
            "[server]\nlisten = [\"127.0.0.1:47124\"]\ncontent = {:?}\n",
            content.to_str().unwrap()
        ),
    )
    .unwrap();
    let (code, out, err) = run(&["run"], &dir, None);
    assert_eq!(code, 3, "{out}{err}");
}

/// §4.1: a `[movement]` relaxation beyond the protocol's bounds stops `run` with status 2;
/// it is never clamped.
#[test]
fn run_refuses_out_of_range_movement_with_status_2() {
    let tmp = tempfile::tempdir().unwrap();
    for (n, line) in ["settle_grace_s = 11.0", "slide_reserve = 150.0"]
        .iter()
        .enumerate()
    {
        let dir = tmp.path().join(format!("data{n}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("server.toml"),
            format!("[server]\nlisten = [\"127.0.0.1:47125\"]\n[movement]\n{line}\n"),
        )
        .unwrap();
        let (code, out, err) = run(&["run"], &dir, None);
        assert_eq!(code, 2, "{line}: {out}{err}");
        let key = line.split(' ').next().unwrap();
        assert!(out.contains(key) || err.contains(key), "{line}: {out}{err}");
    }
}

/// Real installation (read-only): the guard refuses a data directory or content file in it.
/// Only directory names are listed; nothing is opened or written.
#[test]
fn real_install_is_refused() {
    let Some(game) = std::env::var_os("TESSERA_GAME_DIR") else {
        eprintln!("TESSERA_GAME_DIR not set: skipped");
        return;
    };
    let game = std::path::PathBuf::from(game);
    let candidate = game.join("tessera-server-must-not-exist");
    assert!(datadir::install_guard_hit(&candidate));
    let e = DataDir::new(&candidate).create().unwrap_err();
    assert_eq!(e.exit_status(), 4);
    assert!(!candidate.exists());
    let (_, f) = tessera_server::content::check_file(&game.join("world.toml"), true);
    assert_eq!(f[0].rule, tessera_server::tomlx::Rule::InstallPath);
}

/// `run` in the foreground: starts, logs the fingerprint and "ready", and on SIGTERM shuts
/// down cleanly with status 0, saving the world state.
#[cfg(unix)]
#[test]
fn run_and_terminate() {
    use std::io::{BufRead, BufReader};
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("data");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("server.toml"),
        "[server]\nshutdown_grace_s = 0\n[accounts]\nargon2_memory_kib = 19456\nargon2_iterations = 2\nargon2_lanes = 1\nhash_workers = 1\n",
    )
    .unwrap();
    let port = 20_000 + (std::process::id() % 20_000) as u16;
    let mut child = bin()
        .arg("--data-dir")
        .arg(&dir)
        .args(["run", "--listen", &format!("127.0.0.1:{port}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stderr = child.stderr.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let _ = tx.send(line);
        }
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let mut seen = Vec::new();
    while !seen.iter().any(|l: &String| l.contains("ready")) {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        match rx.recv_timeout(left) {
            Ok(l) => seen.push(l),
            Err(_) => {
                let _ = child.kill();
                panic!("no ready line: {seen:?}");
            }
        }
    }
    assert!(seen.iter().any(|l| l.contains("sha256:")), "{seen:?}");
    assert!(
        seen.iter()
            .any(|l| l.contains(&format!("tessera://127.0.0.1:{port}?pin=sha256:")))
    );
    let status = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(status.success());
    let out = child.wait().unwrap();
    assert_eq!(out.code(), Some(0));
    assert!(dir.join("world-state.toml").exists());
    // The lock is released: another exclusive command works.
    assert!(datadir::ServerLock::acquire(&DataDir::new(&dir), false).is_ok());
}

/// `character list`, `move` and `remove` (§2.5), which had no test.
#[test]
fn characters_from_the_command_line() {
    use tessera_protocol::types::{Appearance, Body, Location};
    use tessera_server::characters::{
        CharacterFile, CharacterRecord, StoredAppearance, StoredPosition,
    };
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("data");
    fast_config(&dir);
    std::fs::create_dir_all(dir.join("characters")).unwrap();
    let file = CharacterFile {
        account: "alice".into(),
        characters: vec![CharacterRecord {
            name: "Aldith".into(),
            created: "2026-10-07T00:00:00Z".into(),
            instance: 1,
            position: Some(StoredPosition {
                location: Location::new(1, 1000.0, 1000.0, 5.0),
                heading: 0.0,
                anchor: None,
            }),
            appearance: StoredAppearance::Valid(Appearance::new(Body::Character {
                species: 1,
                sex: 1,
            })),
            look: tessera_protocol::types::Look::none(),
        }],
    };
    std::fs::write(dir.join("characters/alice.toml"), file.to_toml()).unwrap();
    let (code, out, _) = run(&["character", "list"], &dir, None);
    assert_eq!(code, 0);
    assert_eq!(out.trim(), "Aldith\talice\t1\t1\t(6, 6)");
    assert_eq!(
        run(
            &["character", "move", "aldith", "--spawn", "nowhere"],
            &dir,
            None
        )
        .0,
        2
    );
    let (code, _, err) = run(
        &["character", "move", "aldith", "--spawn", "start"],
        &dir,
        None,
    );
    assert_eq!(code, 0, "{err}");
    let back = tessera_server::characters::read_file(&DataDir::new(&dir), "alice").unwrap();
    let p = back.characters[0].position.unwrap();
    assert_eq!(p.location, Location::new(1, 20_240.0, 22_420.0, 440.0));
    // Refused while a server holds the lock (status 4).
    let lock = datadir::ServerLock::acquire(&DataDir::new(&dir), true).unwrap();
    assert_eq!(
        run(&["character", "remove", "Aldith", "--yes"], &dir, None).0,
        4
    );
    drop(lock);
    assert_eq!(run(&["character", "remove", "Aldith"], &dir, None).0, 2);
    assert_eq!(
        run(&["character", "remove", "Aldith", "--yes"], &dir, None).0,
        0
    );
    let (code, out, _) = run(&["character", "list"], &dir, None);
    assert_eq!(code, 0);
    assert!(out.trim().is_empty(), "{out}");
}

/// Finding: `fingerprint --connect` printed a port twice when it was not adjacent in
/// `listen`.
#[test]
fn connect_strings_once_per_port() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("data");
    assert_eq!(run(&["keygen"], &dir, None).0, 0);
    std::fs::write(
        dir.join("server.toml"),
        "[server]\nlisten = [\"127.0.0.1:47123\", \"127.0.0.2:47124\", \"127.0.0.3:47123\"]\n",
    )
    .unwrap();
    let (code, out, err) = run(&["fingerprint", "--connect", "host.lan"], &dir, None);
    assert_eq!(code, 0, "{err}");
    let ports: Vec<&str> = out
        .lines()
        .map(|l| l.split(':').nth(2).unwrap().split('?').next().unwrap())
        .collect();
    assert_eq!(ports, vec!["47123", "47124"]);
}
