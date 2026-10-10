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
use tessera_protocol::codes::AppErrorCode;
use tessera_protocol::fingerprint::Fingerprint;
use tessera_protocol::message::{Credentials, Disconnect, Hello, Message};

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

/// Finding: the statistics line reported the longest tick since start and cumulative bytes;
/// §11.1 asks for the longest tick over the interval and bytes per second.
#[test]
fn statistics_line_reports_interval_figures() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("data");
    write_config(&dir, "[log]\nstats_interval_s = 1\n");
    let mut cmd = bin();
    cmd.arg("--data-dir")
        .arg(&dir)
        .args(["run", "--listen", &format!("127.0.0.1:{}", port(3))]);
    let (child, mut lines, rx) = start(&mut cmd);
    let deadline = Instant::now() + Duration::from_secs(20);
    while !lines.iter().any(|l| l.contains("stats")) {
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(l) => lines.push(l),
            Err(_) => break,
        }
    }
    let (code, _) = terminate(child, &rx);
    assert_eq!(code, Some(0));
    let line = strip_ansi(
        lines
            .iter()
            .find(|l| l.contains("stats"))
            .unwrap_or_else(|| panic!("no statistics line: {lines:?}")),
    );
    // Every §11.1 field (resident memory where the platform reports it).
    for field in [
        "connections=",
        "logged_in=",
        "in_world=",
        "per_instance=",
        "entities=",
        "interest_pairs=",
        "tick_last_us=",
        "tick_max_us=",
        "overruns=",
        "bytes_in_per_s=",
        "bytes_out_per_s=",
        "accounted_bytes=",
    ] {
        assert!(line.contains(field), "{field} missing: {line}");
    }
    #[cfg(target_os = "linux")]
    assert!(
        line.contains("resident_bytes="),
        "resident_bytes missing: {line}"
    );
    for field in ["bytes_in=", "bytes_out="] {
        assert!(!line.contains(field), "cumulative {field} logged: {line}");
    }
}

/// §11.1 (revised 2026-10-10), §15.3: the "connection closed" line carries the connection's
/// own bytes in and out, totals from acceptance to end, counted as the statistics line
/// counts them (frames and datagrams; not the kind byte of a server-opened stream).
#[test]
fn closed_line_carries_the_connections_own_bytes() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("data");
    write_config(&dir, "");
    let mut add = bin();
    add.arg("--data-dir")
        .arg(&dir)
        .args(["account", "add", "carol", "--password-stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = add.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"carol test pass\n")
        .unwrap();
    assert!(child.wait().unwrap().success());
    let p = port(4);
    let mut cmd = bin();
    cmd.arg("--data-dir")
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
    // (client address, bytes it sent, bytes it received) per connection.
    let clients: Vec<(String, u64, u64)> = rt.block_on(async {
        // One connection logs in (control and chat streams), one only greets.
        let mut a = RawClient::connect(addr, fp).await;
        a.hello().await;
        a.login("carol", "carol test pass").await.unwrap();
        let mut b = RawClient::connect(addr, fp).await;
        b.hello().await;
        let mut out = Vec::new();
        for c in [&mut a, &mut b] {
            // Everything the server sent has arrived before the client closes.
            c.drain(Duration::from_millis(500)).await;
            let (sent, received) = c.byte_counts();
            out.push((c.endpoint.local_addr().unwrap().to_string(), sent, received));
            c.conn.close(0u32.into(), b"");
            c.endpoint.wait_idle().await;
        }
        out
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    while lines
        .iter()
        .filter(|l| l.contains("connection closed"))
        .count()
        < clients.len()
    {
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(l) => lines.push(l),
            Err(_) => break,
        }
    }
    let (code, _) = terminate(child, &rx);
    assert_eq!(code, Some(0));
    for (client_addr, sent, received) in &clients {
        let line = strip_ansi(
            lines
                .iter()
                .find(|l| l.contains("connection closed") && l.contains(client_addr.as_str()))
                .unwrap_or_else(|| panic!("no closed line for {client_addr}: {lines:?}")),
        );
        let field = |name: &str| -> u64 {
            let key = format!("{name}=");
            let at = line
                .find(&key)
                .unwrap_or_else(|| panic!("{name} missing: {line}"));
            line[at + key.len()..]
                .chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .unwrap_or_else(|_| panic!("{name} is not a number: {line}"))
        };
        assert!(*sent > 0 && *received > 0, "{client_addr} exchanged bytes");
        assert_eq!(
            field("bytes_in"),
            *sent,
            "bytes in of {client_addr}: {line}"
        );
        assert_eq!(
            field("bytes_out"),
            *received,
            "bytes out of {client_addr}: {line}"
        );
    }
    assert_ne!(
        (clients[0].1, clients[0].2),
        (clients[1].1, clients[1].2),
        "the two connections exchanged different amounts"
    );
}

/// Waits (without blocking the client runtime) for the "connection closed" line of the
/// client at `client_addr`, keeping every line read in `lines`.
async fn closed_line(rx: &Receiver<String>, lines: &mut Vec<String>, client_addr: &str) -> String {
    let find = |lines: &[String]| {
        lines
            .iter()
            .find(|l| l.contains("connection closed") && l.contains(client_addr))
            .map(|l| strip_ansi(l))
    };
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(l) = find(lines) {
            return l;
        }
        assert!(
            Instant::now() < deadline,
            "no closed line for {client_addr}: {lines:?}"
        );
        while let Ok(l) = rx.try_recv() {
            lines.push(l);
        }
        if find(lines).is_none() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

fn hello_message() -> Message {
    Message::Hello(Hello {
        minor_max: 0,
        client_caps: 0,
        max_entities: 1024,
        client_name: "closed-line test".into(),
    })
}

/// The reason phrase of a peer's close that tries to end the field and imitate others.
const HOSTILE_REASON: &[u8] = b"a\"b\\c\nend=server close=\"x\"";

/// §11.1 (sixth revision, 2026-10-10), §14 invariant 11: the closed line's `code`,
/// `reason`, `end` and `close` record the cause of the end, in the stated order, for each
/// closing path the server takes: a peer's `Disconnect` after `Hello` (logout), a peer's
/// close without `Disconnect` (a defined code, and an undefined one with a reason phrase
/// that tries to end the field), a peer's `Disconnect` before `Hello`, a server
/// `Disconnect` acknowledged by the peer and one whose linger passed (both
/// `SERVER_FULL`), and an immediate close on a protocol violation. `reason` and `close`
/// are quoted and escaped, `code` and `end` plain. The idle timeout, the fallback close
/// without a queued `Disconnect` and the transport's description are covered by the unit
/// tests of `closing.rs`; the endpoint close at shutdown by
/// `closed_line_records_the_endpoint_close_at_shutdown`.
#[test]
fn closed_line_records_the_cause_of_the_end() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("data");
    write_config(&dir, "[limits]\nmax_connections = 1\n");
    let p = port(5);
    let mut cmd = bin();
    cmd.arg("--data-dir")
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
    let local = |c: &RawClient| c.endpoint.local_addr().unwrap().to_string();
    let checks: Vec<(&str, String)> = rt.block_on(async {
        let mut out = Vec::new();
        // 1. Logout: the peer's Disconnect, answered by the server's NO_ERROR close.
        let mut d = RawClient::connect(addr, fp).await;
        d.hello().await;
        d.send(&Message::Disconnect(Disconnect {
            code: AppErrorCode::NoError,
            text: "bye".into(),
        }))
        .await;
        assert!(d.closed_code(WAIT).await.is_some());
        out.push(("logout", closed_line(&rx, &mut lines, &local(&d)).await));
        // 2. The peer closes without a Disconnect.
        let mut e = RawClient::connect(addr, fp).await;
        e.hello().await;
        e.conn.close(0x0Du32.into(), b"client crashed");
        e.endpoint.wait_idle().await;
        out.push(("peer close", closed_line(&rx, &mut lines, &local(&e)).await));
        // 2b. The same with a code §14.1 does not define and a hostile reason phrase.
        let mut f = RawClient::connect(addr, fp).await;
        f.hello().await;
        f.conn.close(0x1_0000u32.into(), HOSTILE_REASON);
        f.endpoint.wait_idle().await;
        out.push((
            "peer close, odd code",
            closed_line(&rx, &mut lines, &local(&f)).await,
        ));
        // 2c. A Disconnect before Hello: a protocol violation, closed at once by the server.
        let mut g = RawClient::connect(addr, fp).await;
        g.send(&Message::Disconnect(Disconnect {
            code: AppErrorCode::NoError,
            text: "early".into(),
        }))
        .await;
        assert_eq!(
            g.closed_code(WAIT).await,
            Some(u64::from(AppErrorCode::ProtocolViolation.to_u16()))
        );
        out.push((
            "early disconnect",
            closed_line(&rx, &mut lines, &local(&g)).await,
        ));
        // A holds the one place.
        let mut a = RawClient::connect(addr, fp).await;
        a.hello().await;
        // 3. SERVER_FULL, acknowledged by the peer's NO_ERROR close.
        let mut b = RawClient::connect(addr, fp).await;
        b.send(&hello_message()).await;
        assert_eq!(
            b.disconnect_code(WAIT).await,
            Some(AppErrorCode::ServerFull)
        );
        b.conn.close(0u32.into(), b"");
        b.endpoint.wait_idle().await;
        out.push((
            "full, acknowledged",
            closed_line(&rx, &mut lines, &local(&b)).await,
        ));
        // 4. SERVER_FULL, and the peer does not answer: the server closes after the linger.
        let mut c = RawClient::connect(addr, fp).await;
        c.send(&hello_message()).await;
        assert_eq!(
            c.disconnect_code(WAIT).await,
            Some(AppErrorCode::ServerFull)
        );
        assert!(c.closed_code(WAIT).await.is_some());
        out.push((
            "full, linger",
            closed_line(&rx, &mut lines, &local(&c)).await,
        ));
        // 5. A protocol violation (a second Hello): the server closes at once.
        a.send(&hello_message()).await;
        assert_eq!(
            a.closed_code(WAIT).await,
            Some(u64::from(AppErrorCode::ProtocolViolation.to_u16()))
        );
        out.push(("violation", closed_line(&rx, &mut lines, &local(&a)).await));
        out
    });
    let (code, _) = terminate(child, &rx);
    assert_eq!(code, Some(0));
    let expect = [
        (
            "logout",
            "code=NO_ERROR",
            "reason=\"bye\"",
            "end=peer",
            Some("close=\"NO_ERROR from server\""),
        ),
        (
            "peer close",
            "code=INTERNAL_ERROR",
            "reason=\"client crashed\"",
            "end=peer",
            None,
        ),
        (
            "peer close, odd code",
            "code=code 65536",
            r#"reason="a\"b\\c\nend=server close=\"x\"" end=peer"#,
            "end=peer",
            None,
        ),
        (
            "early disconnect",
            "code=PROTOCOL_VIOLATION",
            "reason=",
            "end=server",
            None,
        ),
        (
            "full, acknowledged",
            "code=SERVER_FULL",
            "reason=\"\"",
            "end=server",
            Some("close=\"NO_ERROR from peer\""),
        ),
        (
            "full, linger",
            "code=SERVER_FULL",
            "reason=\"\"",
            "end=server",
            Some("close=\"SERVER_FULL from server\""),
        ),
        (
            "violation",
            "code=PROTOCOL_VIOLATION",
            "reason=",
            "end=server",
            None,
        ),
    ];
    for ((name, line), (want, code, reason, end, close)) in checks.iter().zip(expect) {
        assert_eq!(*name, want);
        for part in [code, reason, end] {
            assert!(line.contains(part), "{name}: {part} missing: {line}");
        }
        match close {
            Some(c) => assert!(line.contains(c), "{name}: {c} missing: {line}"),
            // The reason may itself contain the text " close=" (quoted, so it is no field).
            None => assert!(
                !line.replace(reason, "").contains(" close="),
                "{name}: close field: {line}"
            ),
        }
        assert!(!line.contains('\n'), "{name}: one line: {line}");
    }
}

/// Starts `run` on `port` with `extra` configuration and returns the child, the lines so
/// far, the line channel, the pinned fingerprint and the address.
fn start_server(
    dir: &Path,
    extra: &str,
    port: u16,
) -> (
    Child,
    Vec<String>,
    Receiver<String>,
    Fingerprint,
    std::net::SocketAddr,
) {
    write_config(dir, extra);
    let mut cmd = bin();
    cmd.arg("--data-dir")
        .arg(dir)
        .args(["run", "--listen", &format!("127.0.0.1:{port}")]);
    let (child, lines, rx) = start(&mut cmd);
    let fp_line = lines.iter().find(|l| l.contains("sha256:")).unwrap();
    let at = fp_line.find("sha256:").unwrap();
    let fp: Fingerprint = fp_line[at..at + 71].parse().unwrap();
    let addr = format!("127.0.0.1:{port}").parse().unwrap();
    (child, lines, rx, fp, addr)
}

/// SIGTERM from a blocking task, so the clients of `rt` keep running (and stay connected)
/// while the server shuts down; returns the exit status and the remaining lines.
async fn terminate_async(child: Child, rx: Receiver<String>) -> (Option<i32>, Vec<String>) {
    tokio::task::spawn_blocking(move || terminate(child, &rx))
        .await
        .unwrap()
}

/// §11.1 "Graceful shutdown" (sixth revision), §14 invariant 11, §15.3: a connection that
/// the endpoint close of §12.1 step 6 ends is logged as closed by the server with
/// `SERVER_SHUTDOWN`, never with the transport's local-close text: without a `Disconnect`
/// (a connection refused for a full server that has not sent `Hello`, which the step-5
/// message does not reach) under rule 2, and after the step-5 `Disconnect` that the peer
/// does not acknowledge with `close` = `SERVER_SHUTDOWN from server`.
#[test]
fn closed_line_records_the_endpoint_close_at_shutdown() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("data");
    let (child, lines, rx, fp, addr) =
        start_server(&dir, "[limits]\nmax_connections = 1\n", port(6));
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = |c: &RawClient| c.endpoint.local_addr().unwrap().to_string();
    let (code, rest, a_addr, b_addr) = rt.block_on(async {
        // A holds the one place and is told SERVER_SHUTDOWN; it never answers.
        let mut a = RawClient::connect(addr, fp).await;
        a.hello().await;
        // B is past the handshake but refused (no place) and silent: no Disconnect.
        let b = RawClient::connect(addr, fp).await;
        let (code, rest) = terminate_async(child, rx).await;
        let (a_addr, b_addr) = (local(&a), local(&b));
        drop((a, b));
        (code, rest, a_addr, b_addr)
    });
    assert_eq!(code, Some(0));
    let all: Vec<String> = lines
        .into_iter()
        .chain(rest)
        .map(|l| strip_ansi(&l))
        .collect();
    let find = |client: &str| {
        all.iter()
            .find(|l| l.contains("connection closed") && l.contains(client))
            .unwrap_or_else(|| panic!("no closed line for {client}: {all:?}"))
            .clone()
    };
    let a = find(&a_addr);
    for part in [
        "code=SERVER_SHUTDOWN",
        "reason=\"\"",
        "end=server",
        "close=\"SERVER_SHUTDOWN from server\"",
    ] {
        assert!(a.contains(part), "disconnected: {part} missing: {a}");
    }
    let b = find(&b_addr);
    for part in [
        "code=SERVER_SHUTDOWN",
        "reason=\"SERVER_SHUTDOWN\"",
        "end=server",
    ] {
        assert!(b.contains(part), "no disconnect: {part} missing: {b}");
    }
    assert!(!b.contains(" close="), "no disconnect: close field: {b}");
    // Never the transport's generic local-close text, as cause or as `close`.
    let local_text = quinn::ConnectionError::LocallyClosed.to_string();
    for l in [&a, &b] {
        for field in ["reason", "close"] {
            let bad = format!("{field}={local_text:?}");
            assert!(!l.contains(&bad), "transport text: {l}");
        }
    }
}

/// §11.1 "Fields of the cause": in the `json` format every field of the cause is a JSON
/// string holding the value itself (no quotes of the text format inside), with a hostile
/// reason phrase escaped by JSON; `close` is absent unless the cause is a `Disconnect`.
#[test]
fn closed_line_cause_fields_in_json() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("data");
    let (child, mut lines, rx, fp, addr) = start_server(
        &dir,
        "[limits]\nmax_connections = 1\n[log]\nformat = \"json\"\n",
        port(7),
    );
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = |c: &RawClient| c.endpoint.local_addr().unwrap().to_string();
    let (logout, hostile, shutdown, code) = rt.block_on(async {
        let mut d = RawClient::connect(addr, fp).await;
        d.hello().await;
        d.send(&Message::Disconnect(Disconnect {
            code: AppErrorCode::NoError,
            text: "bye".into(),
        }))
        .await;
        assert!(d.closed_code(WAIT).await.is_some());
        let logout = closed_line(&rx, &mut lines, &local(&d)).await;
        let mut e = RawClient::connect(addr, fp).await;
        e.hello().await;
        e.conn.close(0x1_0000u32.into(), HOSTILE_REASON);
        e.endpoint.wait_idle().await;
        let hostile = closed_line(&rx, &mut lines, &local(&e)).await;
        // A holds the place; B is refused and silent until the endpoint close.
        let mut a = RawClient::connect(addr, fp).await;
        a.hello().await;
        let b = RawClient::connect(addr, fp).await;
        let b_addr = local(&b);
        let (code, rest) = terminate_async(child, rx).await;
        drop((a, b));
        let shutdown = rest
            .into_iter()
            .find(|l| l.contains("connection closed") && l.contains(&b_addr))
            .unwrap_or_else(|| panic!("no closed line for {b_addr}"));
        (logout, hostile, shutdown, code)
    });
    assert_eq!(code, Some(0));
    for part in [
        r#""code":"NO_ERROR""#,
        r#""reason":"bye""#,
        r#""end":"peer""#,
        r#""close":"NO_ERROR from server""#,
    ] {
        assert!(logout.contains(part), "logout: {part} missing: {logout}");
    }
    for part in [
        r#""code":"code 65536""#,
        r#""reason":"a\"b\\c\nend=server close=\"x\"""#,
        r#""end":"peer""#,
    ] {
        assert!(hostile.contains(part), "hostile: {part} missing: {hostile}");
    }
    assert!(
        !hostile.contains(r#""close":"#),
        "hostile: close field: {hostile}"
    );
    for part in [
        r#""code":"SERVER_SHUTDOWN""#,
        r#""reason":"SERVER_SHUTDOWN""#,
        r#""end":"server""#,
    ] {
        assert!(
            shutdown.contains(part),
            "shutdown: {part} missing: {shutdown}"
        );
    }
    assert!(
        !shutdown.contains(r#""close":"#),
        "shutdown: close field: {shutdown}"
    );
}

/// The line without terminal colour sequences (ESC `[` … final letter).
fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            for d in chars.by_ref() {
                if d.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}
