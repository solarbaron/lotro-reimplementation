// SPDX-License-Identifier: GPL-3.0-or-later
//! The datagram dump (`super::super::dump`) and the login server switch
//! (`super::super::diag` "The login server switch") on the synthetic service: the files hold
//! the first datagram as sent with every credential replaced, the endpoint and the counts at
//! close; nothing is written without the switch or to a refused directory; no log line names
//! a path, the account or the ticket.

use std::ffi::OsStr;
use std::path::Path;

use super::diagnostics::{assert_no_endpoint, logs_of, run_until};
use super::*;
use crate::connector::diag::{LOGIN_SERVER_ENV, LoginServerEntry, login_server_entry};
use crate::connector::dump::{
    DUMP_DIR_ENV, DumpDir, DumpKind, Endpoint, MAX_DUMPS_PER_KIND, REDACTED, Target, write_first,
};
use crate::session::{AuthConnection, FLAG_SESSION_AUTH, SessionAuthHeader};

/// The subscription (account) name of [`session_start`].
const SUB: &str = "SUB0000000000000000000000";

fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn dir_accepted(dir: &Path, install: &Path) -> DumpDir {
    DumpDir::accept(Some(dir.as_os_str()), install).expect("accepted")
}

/// The names in `dir`, sorted.
fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .expect("dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

/// No credential of the synthetic session in `bytes`.
fn assert_no_credentials(bytes: &[u8]) {
    for needle in [
        TICKET.as_bytes().to_vec(),
        SUB.as_bytes().to_vec(),
        utf16(SUB),
        ACCOUNT.to_vec(),
        PASSWORD.to_vec(),
    ] {
        assert!(
            find(bytes, &needle).is_none(),
            "a credential was written: {}",
            String::from_utf8_lossy(&needle)
        );
    }
}

/// A session start that dumps into `dir` (install at `install`), the login server being
/// entry `entry`.
fn dumping_start(
    dir: &Path,
    install: &Path,
    entry: LoginServerEntry,
    chat: Option<SocketAddrV4>,
) -> SessionStart {
    let mut start = session_start(0, chat, Arc::default());
    start.dump = Some(dir_accepted(dir, install));
    start.login_server_entry = entry;
    start
}

fn start_driver(start: SessionStart) -> (World, SessionDriver<FakeNet>) {
    let world = new_world(Service::default());
    let mut net = FakeNet(world.clone());
    let d = SessionDriver::start(&mut net, start, 0.0, &go).expect("start");
    (world, d)
}

#[test]
fn the_login_datagram_is_written_as_sent_with_its_credentials_replaced() {
    let dir = tempfile::tempdir().expect("dir");
    let install = tempfile::tempdir().expect("install");
    let entry = LoginServerEntry {
        index: 1,
        count: 4,
        overridden: true,
    };
    let ((sent, ended), logs) = logs_of(|| {
        let (world, mut d) = start_driver(dumping_start(dir.path(), install.path(), entry, None));
        run_until(&mut d, 0.0, 21.0);
        (world.take_sent(), d.ended())
    });
    assert!(ended, "the handshake timed out");
    assert_eq!(
        names(dir.path()),
        ["login-first-datagram.bin", "login-first-datagram.txt"]
    );
    let (first, to) = sent.first().cloned().expect("a datagram was sent");
    assert_eq!(to, LOGIN_SERVER);
    let bin = std::fs::read(dir.path().join("login-first-datagram.bin")).expect("bin");
    assert_eq!(bin.len(), first.len());
    assert_no_credentials(&bin);
    // Exactly the account's UTF-16LE text and the ticket text are replaced; every other
    // byte, the checksum included, is the one sent.
    let account_at = find(&first, &utf16(SUB)).expect("account in the sent datagram");
    let ticket_at = find(&first, TICKET.as_bytes()).expect("ticket in the sent datagram");
    let account = account_at..account_at + utf16(SUB).len();
    let ticket = ticket_at..ticket_at + TICKET.len();
    for (i, (b, s)) in bin.iter().zip(&first).enumerate() {
        if account.contains(&i) || ticket.contains(&i) {
            assert_eq!(*b, REDACTED, "byte {i} not replaced");
        } else {
            assert_eq!(b, s, "byte {i} differs");
        }
    }
    let txt = std::fs::read_to_string(dir.path().join("login-first-datagram.txt")).expect("txt");
    assert_no_credentials(txt.as_bytes());
    for part in [
        "FOR THE OWNER AND THE SPEC WRITER ONLY",
        "wirePrefix (u16 BE)",
        "0x00010000 (65536): SessionAuthHeader (0x00010000)",
        "seqNum (u32 BE)",
        "checksum (u32 BE)",
        "as sent, computed over the original bytes",
        "the server's\naddress and port and the local address and port used",
        "depends on the credentials",
        "netVersion length (VariableU32)",
        "\"180804_netver:9999; didver:00000000-0000-0000-0000-000000000000\"",
        "authBlockLen (u32 LE)",
        "HasExtraBlob (0x04)",
        "clientTimestamp",
        "accountOrTicketStr (UTF-16LE)",
        "ticket text (ASCII)",
        "redacted: 16 characters",
        "Bytes after the optional headers: 0",
        "entry 2 of 4 (override) of the world status's <loginservers> (chosen by \
         TESSERA_OFFICIAL_LOGIN_SERVER)",
        "remote address and port used: 192.0.2.10:9001",
        "remote address range: documentation range (RFC 5737)",
        "remote host: name resolved (first IPv4 address)",
        "local address and port bound: 192.0.2.1:5000",
        "local address source: wildcard (0.0.0.0)",
        "bind case (§3.2): Case B",
        "clientWord16 (u16 BE)",
        "peerWord18 (u16 BE)",
        "At close",
        "reason: HandshakeTimeout",
        "state: SessionAuthSent",
        "datagrams sent: 10",
        "SessionAuthHeader or ReferralAuthHeader datagrams sent: 10\n    of those, retransmits: 9",
        "datagrams received from the server: 0",
    ] {
        assert!(txt.contains(part), "{part} missing:\n{txt}");
    }
    for (field, value) in [
        ("  0x0006      4  flags (u32 BE)", "0x00010000 (65536)"),
        ("authType (u32 LE)", "0x00000001 (1)"),
        (
            "accountOrTicketStr (UTF-16LE)",
            &format!("redacted: the account name, {} UTF-16 units", SUB.len()),
        ),
        ("extraBlobLen (u32 LE)", "0x00000011 (17)"),
        (
            "ticket length (VariableU32, ASCII characters)",
            "0x0010 (16)",
        ),
    ] {
        let line = txt
            .lines()
            .find(|l| l.contains(field))
            .unwrap_or_else(|| panic!("{field} missing:\n{txt}"));
        assert!(line.contains(value), "{value} missing: {line}");
    }
    assert_eq!(txt.matches("At close").count(), 1, "{txt}");
    assert!(
        !txt.contains("CARRIED A PASSWORD"),
        "no password was sent:\n{txt}"
    );
    // One line per file, with the kind and no path; the start line has the netVersion.
    let written: Vec<&str> = logs
        .lines()
        .filter(|l| l.contains("official session: datagram dump written"))
        .collect();
    assert_eq!(written.len(), 2, "{logs}");
    assert!(
        written.iter().all(|l| l.contains("kind=\"login\"")),
        "{logs}"
    );
    assert!(
        logs.contains(
            "net_version=\"180804_netver:9999; didver:00000000-0000-0000-0000-000000000000\""
        ),
        "{logs}"
    );
    let path = dir.path().to_string_lossy();
    assert!(!logs.contains(path.as_ref()), "a path was logged:\n{logs}");
    assert!(!logs.contains("first-datagram"), "a file name was logged");
    assert_no_endpoint(&logs);
    assert_no_credentials(logs.as_bytes());
}

/// The names of the dump files below `dir`, at any depth.
fn dump_files_below(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d)
            .expect("dir")
            .map(|e| e.expect("entry"))
        {
            let name = e.file_name().to_string_lossy().into_owned();
            if e.file_type().expect("type").is_dir() {
                stack.push(e.path());
            } else if name.contains("first-datagram") {
                found.push(name);
            }
        }
    }
    found
}

#[test]
fn nothing_is_written_without_the_switch() {
    assert!(DumpDir::accept(None, Path::new("/")).is_none());
    if std::env::var_os(DUMP_DIR_ENV).is_some() {
        // The environment of this test run turns the dump on, so the default cannot be
        // checked here.
        return;
    }
    // The whole connector thread (`worker::run` reads the variable itself), through the
    // login connection's handshake, the point at which a dump would be written.
    let (cwd_before, logs) = logs_of(|| {
        let cwd_before = dump_files_below(Path::new("."));
        let mut run = start(Service::default(), true);
        let _login = to_character_list(&mut run);
        let _ = run.session.send(Instant::now(), Command::Disconnect);
        run.wait_exit(true);
        assert!(
            dump_files_below(run._dir.path()).is_empty(),
            "a dump was written"
        );
        cwd_before
    });
    assert_eq!(dump_files_below(Path::new(".")), cwd_before);
    assert!(
        logs.contains("official session: handshake started"),
        "the worker's lines were not captured:\n{logs}"
    );
    assert!(!logs.contains("datagram dump"), "{logs}");
}

#[test]
fn a_bad_dump_directory_is_refused_without_naming_it() {
    let dir = tempfile::tempdir().expect("dir");
    let install = tempfile::tempdir().expect("install");
    let inside = install.path().join("canary-inside");
    std::fs::create_dir(&inside).expect("inside");
    let file = dir.path().join("canary-file");
    std::fs::write(&file, b"x").expect("file");
    let missing = dir.path().join("canary-missing");
    // Two work trees: a `.git` directory above the value, and a linked work tree's `.git`
    // file in the value itself.
    let repo = tempfile::tempdir().expect("repo");
    std::fs::create_dir(repo.path().join(".git")).expect(".git");
    let nested = repo.path().join("canary-a").join("canary-b");
    std::fs::create_dir_all(&nested).expect("nested");
    let linked = tempfile::tempdir().expect("linked");
    std::fs::write(linked.path().join(".git"), b"gitdir: elsewhere\n").expect(".git file");
    let ((), logs) = logs_of(|| {
        for value in [
            missing.as_os_str(),
            file.as_os_str(),
            inside.as_os_str(),
            install.path().as_os_str(),
            OsStr::new(""),
            nested.as_os_str(),
            linked.path().as_os_str(),
        ] {
            assert!(DumpDir::accept(Some(value), install.path()).is_none());
        }
        // An install that is not known refuses every directory.
        assert!(DumpDir::accept(Some(dir.path().as_os_str()), Path::new("")).is_none());
    });
    assert_eq!(
        logs.matches("datagram dump directory refused").count(),
        8,
        "{logs}"
    );
    assert!(logs.contains("inside the game install directory"), "{logs}");
    assert_eq!(logs.matches("inside a git work tree").count(), 2, "{logs}");
    assert!(logs.contains("not an existing directory"), "{logs}");
    assert!(logs.contains("reason=\"not a directory\""), "{logs}");
    assert!(!logs.contains("canary"), "a value was logged:\n{logs}");
    for p in [dir.path(), install.path(), repo.path(), linked.path()] {
        assert!(!logs.contains(p.to_string_lossy().as_ref()), "{logs}");
    }
    // Nothing was written anywhere.
    assert_eq!(names(dir.path()), ["canary-file"]);
    assert_eq!(names(install.path()), ["canary-inside"]);
    assert!(names(&inside).is_empty());
    assert!(names(&nested).is_empty());
    assert_eq!(names(linked.path()), [".git"]);
}

#[test]
fn a_password_datagram_is_redacted_and_flagged() {
    let dir = tempfile::tempdir().expect("dir");
    let install = tempfile::tempdir().expect("install");
    let account = "canary-account";
    let password = "canary-password";
    let auth = SessionAuthHeader::for_connection(
        AuthConnection::Primary,
        "9999",
        account,
        Some(TICKET),
        Some(password),
        1_700_000_000,
    )
    .expect("auth");
    let datagram = encode_datagram(
        DatagramHeader {
            flags: FLAG_SESSION_AUTH,
            ..DatagramHeader::default()
        },
        &OptionalHeaders {
            session_auth: Some(auth),
            ..OptionalHeaders::default()
        },
        &[],
        None,
        None,
    )
    .expect("datagram");
    assert!(find(&datagram, &utf16(password)).is_some());
    let endpoint = Endpoint {
        kind: DumpKind::Login,
        target: Target::LoginServer(LoginServerEntry::default()),
        remote_form: "form",
        remote_name: "name",
        local_source: "source",
        bind: "bind",
    };
    let file = write_first(
        &dir_accepted(dir.path(), install.path()),
        &endpoint,
        &datagram,
        LOGIN_SERVER,
        Some(LOCAL),
    );
    assert!(file.is_some(), "written");
    drop(file);
    let bin = std::fs::read(dir.path().join("login-first-datagram.bin")).expect("bin");
    let txt = std::fs::read_to_string(dir.path().join("login-first-datagram.txt")).expect("txt");
    assert_eq!(bin.len(), datagram.len());
    for written in [&bin[..], txt.as_bytes()] {
        assert_no_credentials(written);
        for needle in [utf16(account), utf16(password)] {
            assert!(find(written, &needle).is_none(), "a credential was written");
        }
    }
    for part in [
        "HasSecondString (0x02)",
        "secondStr (UTF-16LE)",
        "redacted: the password",
        "THIS DATAGRAM CARRIED A PASSWORD",
        "never hand over the files",
    ] {
        assert!(txt.contains(part), "{part} missing:\n{txt}");
    }
}

#[test]
fn existing_files_are_kept_and_a_later_connection_takes_the_next_number() {
    let dir = tempfile::tempdir().expect("dir");
    let install = tempfile::tempdir().expect("install");
    let other = tempfile::tempdir().expect("other");
    std::fs::write(dir.path().join("login-first-datagram.bin"), b"keep").expect("old");
    let target = other.path().join("target");
    std::fs::write(&target, b"untouched").expect("target");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, dir.path().join("login-2-first-datagram.txt"))
        .expect("symlink");
    #[cfg(not(unix))]
    std::fs::write(dir.path().join("login-2-first-datagram.txt"), b"keep").expect("old");
    for _ in 0..2 {
        let start = dumping_start(
            dir.path(),
            install.path(),
            LoginServerEntry::default(),
            None,
        );
        let (_world, mut d) = start_driver(start);
        d.step(0.1, true, &go);
    }
    assert_eq!(
        names(dir.path()),
        [
            "login-2-first-datagram.txt",
            "login-3-first-datagram.bin",
            "login-3-first-datagram.txt",
            "login-4-first-datagram.bin",
            "login-4-first-datagram.txt",
            "login-first-datagram.bin",
        ]
    );
    assert_eq!(
        std::fs::read(dir.path().join("login-first-datagram.bin")).expect("old"),
        b"keep"
    );
    assert_eq!(std::fs::read(&target).expect("target"), b"untouched");
    let txt = std::fs::read_to_string(dir.path().join("login-3-first-datagram.txt")).expect("txt");
    assert!(
        txt.contains(
            "entry 1 of 1 of the world status's <loginservers> (official-login.md §3.3: the first)"
        ),
        "{txt}"
    );
    // Dropping the driver closed the connection: its counts are in the file.
    assert!(txt.contains("At close"), "{txt}");
    assert!(
        txt.contains("reason: closed locally in its handshake")
            || txt.contains("reason: the connection was let go"),
        "{txt}"
    );
    const { assert!(MAX_DUMPS_PER_KIND == 999) };
}

#[test]
fn the_world_connection_is_written_with_its_cookie_replaced() {
    let dir = tempfile::tempdir().expect("dir");
    let install = tempfile::tempdir().expect("install");
    let cookie: u64 = 0x1122_3344_5566_7788;
    let start = dumping_start(
        dir.path(),
        install.path(),
        LoginServerEntry::default(),
        None,
    );
    let (world, mut d, login) = driver_at_characters_from(Service::default(), start);
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        1.0,
    )
    .expect("enter");
    d.step(1.0, true, &go);
    let _ = world.take_sent();
    world.deliver(login.referral(cookie, WORLD_SERVER), LOGIN_SERVER);
    d.step(1.1, true, &go);
    let sent = world.take_sent();
    let (first, _) = sent
        .iter()
        .find(|(_, to)| *to == WORLD_SERVER)
        .cloned()
        .expect("the ReferralAuthHeader went out");
    drop(d);
    let bin = std::fs::read(dir.path().join("world-first-datagram.bin")).expect("bin");
    assert_eq!(bin.len(), first.len());
    assert!(
        find(&bin, &cookie.to_le_bytes()).is_none(),
        "the cookie was written"
    );
    assert_eq!(bin.get(22..30), Some(&[REDACTED; 8][..]));
    assert_eq!(bin.get(..22), first.get(..22));
    let txt = std::fs::read_to_string(dir.path().join("world-first-datagram.txt")).expect("txt");
    for part in [
        "ReferralAuthHeader (0x00020000)",
        "referralCookie (u64 LE)",
        "remote address: the WorldReferralHeader's address",
        "remote address and port used: 192.0.2.20:9100",
        "remote form: referral address (IP from the header)",
        "remote host: from the referral header",
        "At close",
    ] {
        assert!(txt.contains(part), "{part} missing:\n{txt}");
    }
    // The login connection's files got their counts when the driver let it go.
    let login_txt =
        std::fs::read_to_string(dir.path().join("login-first-datagram.txt")).expect("txt");
    assert!(login_txt.contains("At close"), "{login_txt}");
    assert!(
        login_txt.contains("ConnectChallengeHeaders acted on: 1"),
        "{login_txt}"
    );
    assert_no_credentials(
        &std::fs::read(dir.path().join("login-first-datagram.bin")).expect("bin"),
    );
}

#[test]
fn the_chat_connection_is_written_with_its_credentials_replaced() {
    let dir = tempfile::tempdir().expect("dir");
    let install = tempfile::tempdir().expect("install");
    let start = dumping_start(
        dir.path(),
        install.path(),
        LoginServerEntry::default(),
        Some(CHAT_SERVER),
    );
    let (world, mut d, mut login) = driver_at_characters_from(
        Service {
            chat_socket: true,
            ..Service::default()
        },
        start,
    );
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        1.0,
    )
    .expect("enter");
    world.deliver(login.event(0x90, &session_ready()), LOGIN_SERVER);
    d.step(1.1, true, &go);
    let (player, name) = d.wants_chat(1.2).expect("the chat session is due");
    let net = FakeNet(world.clone());
    d.attach_chat(
        net.chat_binder(d.first_local_ipv4()).expect("binder"),
        player,
        &name,
        1.2,
        &go,
    );
    d.step(2.3, true, &go);
    let first = world
        .take_sent()
        .into_iter()
        .find(|(_, to)| *to == CHAT_SERVER)
        .map(|(dg, _)| dg)
        .expect("the chat handshake began");
    let bin = std::fs::read(dir.path().join("chat-first-datagram.bin")).expect("bin");
    assert_eq!(bin.len(), first.len());
    assert_no_credentials(&bin);
    assert!(find(&first, TICKET.as_bytes()).is_some());
    drop(d);
    let txt = std::fs::read_to_string(dir.path().join("chat-first-datagram.txt")).expect("txt");
    assert_no_credentials(txt.as_bytes());
    for part in [
        "\"180804_chat_fe_2_3\"",
        "0x40000002 (1073741826)",
        "remote address: the chat server",
        "remote address and port used: 192.0.2.30:9200",
        "At close",
        "SessionAuthHeader or ReferralAuthHeader datagrams sent: 1\n    of those, retransmits: 0",
    ] {
        assert!(txt.contains(part), "{part} missing:\n{txt}");
    }
}

#[test]
fn the_login_server_index_chooses_an_entry_and_bad_values_are_refused() {
    let entry = |v: Option<&str>, n| login_server_entry(v.map(OsStr::new), n);
    assert_eq!(
        entry(None, 4),
        LoginServerEntry {
            index: 0,
            count: 4,
            overridden: false
        }
    );
    assert_eq!(entry(None, 4).label(), "1 of 4");
    let two = entry(Some("2"), 4);
    assert_eq!((two.index, two.overridden), (1, true));
    assert_eq!(two.label(), "2 of 4 (override)");
    assert_eq!(entry(Some("4"), 4).index, 3);
    let (refused, logs) = logs_of(|| {
        [
            "0",
            "5",
            " 2",
            "2 ",
            "+2",
            "-1",
            "2.0",
            "",
            "canary",
            "99999999999999999999999",
        ]
        .into_iter()
        .map(|v| entry(Some(v), 4))
        .collect::<Vec<_>>()
    });
    assert!(
        refused.iter().all(|e| *e
            == LoginServerEntry {
                index: 0,
                count: 4,
                overridden: false
            }),
        "{refused:?}"
    );
    assert_eq!(
        logs.matches("official login: login server index refused")
            .count(),
        10,
        "{logs}"
    );
    assert!(logs.contains(LOGIN_SERVER_ENV), "{logs}");
    assert!(!logs.contains("canary"), "a value was logged:\n{logs}");
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let raw = OsStr::from_bytes(&[b'2', 0xFF]);
        assert_eq!(login_server_entry(Some(raw), 4).index, 0);
    }
}
