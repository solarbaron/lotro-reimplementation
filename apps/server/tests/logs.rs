// SPDX-License-Identifier: GPL-3.0-or-later
//! The `run` command end to end: no secret ever reaches the log, at any level (§11.2, §14
//! invariant 2), and `--content` paths are relative to the working directory (§2.2).

#![cfg(unix)]

mod support;

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use support::*;
use tessera_protocol::fingerprint::Fingerprint;
use tessera_protocol::message::{Credentials, Message};

fn bin() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_tessera-server"));
    c.env_remove("RUST_LOG").env_remove("TESSERA_SERVER_DIR");
    c
}

fn port(offset: u32) -> u16 {
    21_000 + ((std::process::id() * 7 + offset) % 20_000) as u16
}

/// Starts `run` and waits for "ready"; returns the child, its stderr lines so far and the
/// channel with the rest.
fn start(cmd: &mut Command) -> (Child, Vec<String>, Receiver<String>) {
    let mut child = cmd
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
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut seen = Vec::new();
    while !seen.iter().any(|l: &String| l.contains("ready")) {
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(l) => seen.push(l),
            Err(_) => {
                let _ = child.kill();
                panic!("no ready line: {seen:?}");
            }
        }
    }
    (child, seen, rx)
}

/// SIGTERM, then the exit status and every remaining stderr line.
fn terminate(mut child: Child, rx: &Receiver<String>) -> (Option<i32>, Vec<String>) {
    let status = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(status.success());
    let code = child.wait().unwrap().code();
    let mut rest = Vec::new();
    while let Ok(l) = rx.recv_timeout(Duration::from_secs(5)) {
        rest.push(l);
    }
    (code, rest)
}

fn write_config(dir: &Path, extra: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(
        dir.join("server.toml"),
        format!(
            "[server]\nshutdown_grace_s = 0\n{extra}[accounts]\nargon2_memory_kib = 19456\nargon2_iterations = 2\nargon2_lanes = 1\nhash_workers = 1\n"
        ),
    )
    .unwrap();
}

/// Finding (test gap): no test ran the server with trace logging through failed and
/// successful logins and a registration and checked that no password, hash or salt is
/// logged.
#[test]
fn no_secret_reaches_the_log_at_trace_level() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("data");
    write_config(&dir, "registration = true\n");
    let mut add = bin();
    add.arg("--data-dir")
        .arg(&dir)
        .args(["account", "add", "alice", "--password-stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = add.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"alice secret pass\n")
        .unwrap();
    assert!(child.wait().unwrap().success());
    let p = port(1);
    let mut cmd = bin();
    cmd.env("RUST_LOG", "trace")
        .arg("--data-dir")
        .arg(&dir)
        .args(["run", "--listen", &format!("127.0.0.1:{p}")]);
    let (child, mut lines, rx) = start(&mut cmd);
    let fp_line = lines.iter().find(|l| l.contains("sha256:")).unwrap();
    let at = fp_line.find("sha256:").unwrap();
    let fp: Fingerprint = fp_line[at..at + 71].parse().unwrap();
    let addr = format!("127.0.0.1:{p}").parse().unwrap();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let mut c = RawClient::connect(addr, fp).await;
        c.hello().await;
        assert!(c.login("alice", "wrong password one").await.is_err());
        assert!(
            c.login("mistypedsecret", "alice secret pass")
                .await
                .is_err()
        );
        c.login("alice", "alice secret pass").await.unwrap();
        let mut r = RawClient::connect(addr, fp).await;
        r.hello().await;
        r.send(&Message::Register(Credentials {
            account: "bob".into(),
            password: "register secret pw".into(),
        }))
        .await;
        let ok = r
            .wait_for(WAIT, |m| match m {
                Received::Control(Message::LoginOk(_)) => Some(true),
                Received::Control(Message::LoginFailed(_)) => Some(false),
                _ => None,
            })
            .await;
        assert_eq!(ok, Some(true));
    });
    let (code, rest) = terminate(child, &rx);
    assert_eq!(code, Some(0));
    lines.extend(rest);
    let log = lines.join("\n");
    assert!(log.contains("login"), "trace output was captured");
    let accounts = std::fs::read_to_string(dir.join("accounts.toml")).unwrap();
    let mut secrets = vec![
        "wrong password one".to_owned(),
        "alice secret pass".to_owned(),
        "register secret pw".to_owned(),
        "mistypedsecret".to_owned(),
        "$argon2id".to_owned(),
    ];
    for line in accounts.lines().filter(|l| l.starts_with("hash = ")) {
        // The salt and the tag of every stored hash.
        let parts: Vec<&str> = line.trim_end_matches('"').split('$').collect();
        secrets.extend(parts.iter().rev().take(2).map(|s| (*s).to_owned()));
    }
    for s in &secrets {
        assert!(!log.contains(s.as_str()), "{s:?} appears in the log");
    }
}

/// Finding: `run --content FILE` resolved a relative FILE against the data directory, unlike
/// `--config` and `check-content`.
#[test]
fn run_content_path_is_relative_to_the_working_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("data");
    write_config(&dir, "");
    let worlds = tmp.path().join("worlds");
    std::fs::create_dir_all(&worlds).unwrap();
    std::fs::write(
        worlds.join("mine.toml"),
        "format = 1\n[world]\nid = \"mine\"\ntitle = \"Mine\"\nmotd = \"\"\ndefault_spawn = \"s\"\n[[spawn]]\nname = \"s\"\nregion = 1\nlocation = [1000.0, 1000.0, 0.0]\n",
    )
    .unwrap();
    let mut cmd = bin();
    cmd.current_dir(&worlds).arg("--data-dir").arg(&dir).args([
        "run",
        "--listen",
        &format!("127.0.0.1:{}", port(2)),
        "--content",
        "mine.toml",
    ]);
    let (child, lines, rx) = start(&mut cmd);
    assert!(
        lines.iter().any(|l| l.contains("mine")),
        "the file in the working directory is served: {lines:?}"
    );
    let (code, _) = terminate(child, &rx);
    assert_eq!(code, Some(0));
}
