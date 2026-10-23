// SPDX-License-Identifier: GPL-3.0-or-later
//! The gate of `docs/specs/official-entry.md` §6 on the compatibility crate's side: the
//! source scan of OE-T8 (b), the placeholder of OE-T9, the backend's credential rules of
//! §7.3 and the consent proof of §6.1. No test opens a socket or reaches any service (§6.4).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tessera_compat::consent::{self, UserConfirmation, WarningText};
use tessera_compat::net_access::{NetworkAccess, created_count};
use tessera_compat::{
    Availability, CompatBackend, CompatConfig, CompatError, Connector, Credentials, availability,
};
use tessera_net::account::{AccountEvent, LoginRefusal};
use tessera_net::{Backend, Command, DisconnectReason, Event, Session};

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// A source file with its comments removed, as two line-aligned views: `code` with the contents
/// of string and character literals blanked (for braces, items and `use` trees) and `text` with
/// the literals kept (for names written in strings). The literal state carries across lines:
/// multi-line strings, raw strings (`r"…"`, `r#"…"#`) and block comments (nested) are followed.
struct Stripped {
    code: Vec<String>,
    text: Vec<String>,
}

fn strip(src: &str) -> Stripped {
    #[derive(Clone, Copy, PartialEq)]
    enum St {
        Normal,
        Line,
        Block(u32),
        Str,
        Raw(usize),
    }
    let chars: Vec<char> = src.chars().collect();
    let (mut code, mut text) = (String::new(), String::new());
    let mut st = St::Normal;
    let mut i = 0;
    let at = |k: usize| chars.get(k).copied();
    while let Some(c) = at(i) {
        if c == '\n' {
            if st == St::Line {
                st = St::Normal;
            }
            code.push('\n');
            text.push('\n');
            i += 1;
            continue;
        }
        match st {
            St::Line => {}
            St::Block(d) => {
                if c == '*' && at(i + 1) == Some('/') {
                    st = if d <= 1 { St::Normal } else { St::Block(d - 1) };
                    i += 1;
                } else if c == '/' && at(i + 1) == Some('*') {
                    st = St::Block(d + 1);
                    i += 1;
                }
            }
            St::Str => {
                text.push(c);
                if c == '\\' {
                    if let Some(n) = at(i + 1).filter(|n| *n != '\n') {
                        text.push(n);
                        code.push(' ');
                        i += 1;
                    }
                    code.push(' ');
                } else if c == '"' {
                    code.push('"');
                    st = St::Normal;
                } else {
                    code.push(' ');
                }
            }
            St::Raw(h) => {
                text.push(c);
                let closes = c == '"' && (1..=h).all(|k| at(i + k) == Some('#'));
                if closes {
                    code.push('"');
                    for _ in 0..h {
                        code.push('#');
                        text.push('#');
                    }
                    i += h;
                    st = St::Normal;
                } else {
                    code.push(' ');
                }
            }
            St::Normal => {
                let prev_ident =
                    i > 0 && at(i - 1).is_some_and(|p| p.is_alphanumeric() || p == '_');
                if c == '/' && at(i + 1) == Some('/') {
                    st = St::Line;
                } else if c == '/' && at(i + 1) == Some('*') {
                    st = St::Block(1);
                    i += 1;
                } else if c == '"' {
                    code.push('"');
                    text.push('"');
                    st = St::Str;
                } else if c == 'r' && !prev_ident && matches!(at(i + 1), Some('"' | '#')) {
                    let mut h = 0;
                    while at(i + 1 + h) == Some('#') {
                        h += 1;
                    }
                    if at(i + 1 + h) == Some('"') {
                        code.push('r');
                        text.push('r');
                        for _ in 0..h {
                            code.push('#');
                            text.push('#');
                        }
                        code.push('"');
                        text.push('"');
                        i += 1 + h;
                        st = St::Raw(h);
                    } else {
                        code.push(c);
                        text.push(c);
                    }
                } else if c == '\'' {
                    // A character literal ('{', '\'', '\\', '\u{7b}') or a lifetime.
                    let end = if at(i + 1) == Some('\\') {
                        (i + 2..chars.len().min(i + 12)).find(|k| at(*k) == Some('\''))
                    } else if at(i + 2) == Some('\'') {
                        Some(i + 2)
                    } else {
                        None
                    };
                    match end {
                        Some(e) => {
                            code.push_str("' '");
                            for k in i..=e {
                                if let Some(x) = at(k) {
                                    text.push(x);
                                }
                            }
                            i = e;
                        }
                        None => {
                            code.push(c);
                            text.push(c);
                        }
                    }
                } else {
                    code.push(c);
                    text.push(c);
                }
            }
        }
        i += 1;
    }
    Stripped {
        code: code.lines().map(str::to_owned).collect(),
        text: text.lines().map(str::to_owned).collect(),
    }
}

/// The lines of `s` outside `#[cfg(test)]` items, as (function, code, text): the attributed
/// item is skipped by brace depth from the attribute's own line on (further attributes
/// included), or up to its `;`.
fn live_lines(s: &Stripped) -> Vec<(String, String, String)> {
    const QUALIFIERS: &[&str] = &[
        "pub",
        "pub(crate)",
        "pub(super)",
        "const",
        "async",
        "unsafe",
    ];
    let mut out = Vec::new();
    let mut current = String::new();
    let mut i = 0;
    while let Some(code) = s.code.get(i) {
        let t = code.trim_start();
        if let Some(rest) = t.strip_prefix("#[cfg(test)]") {
            let mut depth: i32 = 0;
            let mut opened = false;
            let mut item = rest.to_owned();
            loop {
                depth += item.matches('{').count() as i32 - item.matches('}').count() as i32;
                opened |= item.contains('{');
                if (opened && depth <= 0) || (!opened && item.trim_end().ends_with(';')) {
                    break;
                }
                i += 1;
                match s.code.get(i) {
                    Some(l) => item = l.clone(),
                    None => break,
                }
            }
            i += 1;
            continue;
        }
        if !t.is_empty() {
            if let Some(k) = t.find("fn ") {
                let mut before = t[..k].split_whitespace();
                if before.all(|w| QUALIFIERS.contains(&w)) {
                    current = t[k + 3..]
                        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                        .next()
                        .unwrap_or("")
                        .to_owned();
                }
            }
            let text = s.text.get(i).cloned().unwrap_or_default();
            out.push((current.clone(), code.clone(), text));
        }
        i += 1;
    }
    out
}

/// Every path the `use` declarations (and `extern crate`) of `code` bring in, grouped trees
/// expanded: (path, alias). `use a::{b, c::{d as e}}` gives `a::b` and `a::c::d` as `e`.
fn use_paths(code: &str) -> Vec<(String, Option<String>)> {
    fn expand(prefix: &str, tree: &str, out: &mut Vec<(String, Option<String>)>) {
        let tree = tree.trim();
        if let Some(open) = tree.find('{') {
            let head: String = tree[..open].split_whitespace().collect();
            let inner = tree[open + 1..].trim_end().trim_end_matches('}');
            let mut depth = 0;
            let mut start = 0;
            let mut parts = Vec::new();
            for (k, c) in inner.char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => depth -= 1,
                    ',' if depth == 0 => {
                        parts.push(&inner[start..k]);
                        start = k + 1;
                    }
                    _ => {}
                }
            }
            parts.push(&inner[start..]);
            for p in parts.into_iter().filter(|p| !p.trim().is_empty()) {
                expand(&format!("{prefix}{head}"), p, out);
            }
            return;
        }
        let words: Vec<&str> = tree.split_whitespace().collect();
        let (path, alias) = match words.iter().position(|w| *w == "as") {
            Some(k) => (
                words[..k].concat(),
                words.get(k + 1).map(|a| (*a).to_owned()),
            ),
            None => (words.concat(), None),
        };
        let full = format!("{prefix}{path}");
        let full = full.trim_start_matches("::").to_owned();
        let full = full
            .strip_suffix("::self")
            .map_or(full.clone(), str::to_owned);
        out.push((full, alias));
    }
    let mut out = Vec::new();
    let bytes = code.as_bytes();
    for kw in ["use ", "extern crate "] {
        for (i, _) in code.match_indices(kw) {
            let boundary = i == 0
                || bytes
                    .get(i - 1)
                    .is_some_and(|b| b.is_ascii_whitespace() || b"{};)".contains(b));
            if !boundary {
                continue;
            }
            let rest = &code[i + kw.len()..];
            let Some(end) = rest.find(';') else { continue };
            expand("", &rest[..end], &mut out);
        }
    }
    out
}

/// Whether `path` is `prefix` or below it, segment-wise.
fn under(path: &str, prefix: &str) -> bool {
    path == prefix || path.starts_with(&format!("{prefix}::"))
}

/// The files `#[cfg(test)] mod name;` declarations of `files` point at (test code).
fn test_module_files(files: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for f in files {
        let src = std::fs::read_to_string(f).unwrap_or_default();
        let lines: Vec<&str> = src.lines().map(str::trim).collect();
        for w in lines.windows(2) {
            let (Some(a), Some(b)) = (w.first(), w.get(1)) else {
                continue;
            };
            let decl = if let Some(r) = a.strip_prefix("#[cfg(test)]") {
                if r.trim().is_empty() { *b } else { r.trim() }
            } else {
                continue;
            };
            let Some(name) = decl
                .trim_start_matches("pub(crate) ")
                .trim_start_matches("pub ")
                .strip_prefix("mod ")
                .and_then(|r| r.strip_suffix(';'))
            else {
                continue;
            };
            let Some(dir) = f.parent() else { continue };
            let stem = f.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let base = if matches!(stem, "mod" | "lib" | "main") {
                dir.to_path_buf()
            } else {
                dir.join(stem)
            };
            out.push(base.join(format!("{name}.rs")));
            out.push(base.join(name).join("mod.rs"));
        }
    }
    out
}

/// Whether `src` names `name` as a whole identifier path (`mio` is not `semiotic`).
fn names(src: &str, name: &str) -> bool {
    src.match_indices(name).any(|(i, _)| {
        let before = src[..i].chars().next_back();
        let after = src[i + name.len()..].chars().next();
        let ident = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
        !ident(before) && !ident(after)
    })
}

/// A run-scoped acceptance of the shipped text, written to a temporary directory through the
/// writer (§6.1: how tests obtain a proof).
fn acceptance(dir: &Path) -> consent::Acceptance {
    let (acc, write) = consent::accept(
        Some(dir),
        &WarningText::shipped(),
        UserConfirmation::confirmed_by_user(),
    );
    let _ = write.wait();
    acc
}

/// The `use` rules of OE-T8 (b) for one file's live code: no path below the forbidden
/// modules, however grouped; no renamed or glob import of a crate root that could reach them
/// (`use tessera_net::{self as n}` and then `n::remote`).
fn use_violations(code: &str) -> Vec<String> {
    const FORBIDDEN: &[&str] = &[
        "std::process",
        "core::process",
        "tokio::process",
        "tokio::net",
        "tessera_net::transport",
        "tessera_net::remote",
    ];
    const ROOTS: &[&str] = &["std", "core", "alloc", "tokio", "tessera_net"];
    let mut out = Vec::new();
    for (path, alias) in use_paths(code) {
        if FORBIDDEN.iter().any(|f| under(&path, f)) {
            out.push(format!("use of {path}"));
        }
        if ROOTS.contains(&path.as_str()) && alias.as_deref().is_some_and(|a| a != "_") {
            out.push(format!("renamed crate root {path}"));
        }
        if let Some(base) = path.strip_suffix("::*")
            && ROOTS.contains(&base)
        {
            out.push(format!("glob import of {base}"));
        }
    }
    out
}

#[test]
fn oe_t8b_network_names_only_in_the_network_access_file() {
    // Standard-library and crate network I/O: only in the network-access type's file.
    const SOCKETS: &[&str] = &[
        "UdpSocket",
        "TcpStream",
        "TcpListener",
        "ToSocketAddrs",
        "to_socket_addrs",
        "lookup_host",
        "tokio::net",
        "quinn",
        "socket2",
        "mio",
        "reqwest",
        "ureq",
        "hyper",
    ];
    // tessera-net's own network code (its generic transport has a UDP datagram with no
    // feature gate; feature unification can turn its test-server backend on) and starting
    // other programs: nowhere in this crate.
    const NEVER: &[&str] = &[
        "UdpDatagram",
        "tessera_net::transport",
        "tessera_net::remote",
        "RemoteBackend",
        "std::process",
        "tokio::process",
        "process::Command",
    ];
    let mut files = Vec::new();
    rust_files(&crate_dir().join("src"), &mut files);
    assert!(files.len() >= 8, "{files:?}");
    let tests = test_module_files(&files);
    assert!(
        tests.iter().any(|t| t.ends_with("consent/tests.rs")),
        "{tests:?}"
    );
    let allowed = crate_dir().join("src").join("net_access.rs");
    for f in files.iter().filter(|f| !tests.contains(f)) {
        let live = live_lines(&strip(&std::fs::read_to_string(f).expect("source")));
        let text: String = live.iter().map(|(_, _, t)| format!("{t}\n")).collect();
        let code: String = live.iter().map(|(_, c, _)| format!("{c}\n")).collect();
        for name in NEVER {
            assert!(!names(&text, name), "{} names {name}", f.display());
        }
        let bad = use_violations(&code);
        assert!(bad.is_empty(), "{}: {bad:?}", f.display());
        if f == &allowed {
            continue;
        }
        for name in SOCKETS {
            assert!(!names(&text, name), "{} names {name}", f.display());
        }
    }
    let allowed_src = std::fs::read_to_string(&allowed).expect("source");
    assert!(allowed_src.contains("UdpSocket"));
}

#[test]
fn the_scan_is_not_blinded() {
    // cfg(test) items: one-line ones, attributes after it, braces in literals, a multi-line
    // string and a raw string with braces.
    let src = "fn a() {}\n#[cfg(test)] fn t() { let x = '{'; }\nfn after() { UdpSocket }\n\
               #[cfg(test)]\n#[allow(x)]\nmod m {\n  fn y() { \"}\n}\" }\n  fn z() { r#\"}}\"# }\n}\n\
               fn tail() { /* } */ }\n#[cfg(test)] mod tests;\nfn last() {}\n";
    let live = live_lines(&strip(src));
    let code: Vec<&str> = live.iter().map(|(_, c, _)| c.as_str()).collect();
    let joined = code.join("\n");
    assert!(
        joined.contains("fn after()") && joined.contains("UdpSocket"),
        "{joined}"
    );
    assert!(
        joined.contains("fn tail()") && joined.contains("fn last()"),
        "{joined}"
    );
    assert!(
        !joined.contains("fn t()") && !joined.contains("fn y()") && !joined.contains("fn z()"),
        "{joined}"
    );
    // Grouped and renamed imports.
    for bad in [
        "use std::{io, process::{Command as C}};",
        "use std::process::Command;",
        "use tessera_net::{self as n};",
        "use tessera_net as n;",
        "pub(crate) use ::tessera_net::{account, remote::RemoteConfig};",
        "use tessera_net::{transport};",
        "use tessera_net::*;",
        "extern crate tessera_net as n;",
        "use tokio::{process};",
    ] {
        assert!(!use_violations(bad).is_empty(), "{bad}");
    }
    for ok in [
        "use std::net::{Ipv4Addr, SocketAddrV4};",
        "use tessera_net::account::{AccountEvent, LoginRefusal};",
        "use std::io::Read as _;",
        "use super::*;",
    ] {
        assert!(use_violations(ok).is_empty(), "{ok}");
    }
}

#[test]
fn oe_t8b_manifest_lists_no_network_io_dependency() {
    const NETWORK_CRATES: &[&str] = &[
        "tokio",
        "quinn",
        "socket2",
        "mio",
        "reqwest",
        "ureq",
        "hyper",
        "async-std",
        "smol",
        "curl",
        "isahc",
        "surf",
        "attohttpc",
        "native-tls",
        "rustls",
        "tungstenite",
        "trust-dns",
        "hickory",
    ];
    let manifest = std::fs::read_to_string(crate_dir().join("Cargo.toml")).expect("manifest");
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let Some((name, _)) = line.split_once('=') else {
            continue;
        };
        let name = name.trim();
        for bad in NETWORK_CRATES {
            assert!(!name.starts_with(bad), "Cargo.toml lists {name}");
        }
        if name == "tessera-net" {
            // Only the neutral session model; the test-server transport stays out.
            assert!(line.contains("default-features = false"), "{line}");
        }
    }
}

#[test]
fn oe_t8b_every_backend_constructor_takes_a_proof() {
    let src = std::fs::read_to_string(crate_dir().join("src/backend.rs")).expect("source");
    // `CompatConfig::new` takes the proof by value.
    assert!(src.contains("pub fn new(proof: ConsentProof, credentials: Credentials) -> Self"));
    // Every constructor of the backend takes a `CompatConfig` by value.
    let mut in_backend = false;
    let mut constructors = 0;
    for line in src.lines() {
        if line.starts_with("impl") {
            in_backend = line.contains("CompatBackend<") && !line.contains(" for ");
            continue;
        }
        let t = line.trim();
        if in_backend && t.starts_with("pub fn ") && t.contains("Self") {
            constructors += 1;
            assert!(t.contains("config: CompatConfig"), "{t}");
        }
    }
    assert_eq!(constructors, 2, "new and with_connector");
}

#[test]
fn oe_t9_placeholder_refuses_with_not_available_and_disconnects() {
    assert_eq!(availability(), Availability::NotBuilt);
    let dir = tempfile::tempdir().expect("temp dir");
    let acc = acceptance(dir.path());
    let creds = Credentials::new(b"canary-account", b"canary-password").expect("creds");
    let backend = CompatBackend::new(CompatConfig::new(acc.proof(), creds)).expect("backend");
    assert!(backend.holds_credentials());
    let mut session = Session::new(backend);
    let now = Instant::now();
    session.send(now, Command::Connect).expect("connect");
    let events = session.poll(now);
    assert_eq!(
        events,
        vec![
            Event::Account(AccountEvent::LoginRefused {
                reason: LoginRefusal::NotAvailable,
                retry_after_seconds: None,
                detail: None,
            }),
            Event::Disconnected {
                reason: DisconnectReason::TransportFailed(
                    CompatError::ProtocolNotBuilt.to_string()
                ),
            },
        ]
    );
    assert!(!format!("{session:?}").contains("canary"));
}

#[derive(Debug, Clone, Default)]
struct Recording {
    calls: Arc<Mutex<Vec<(usize, usize)>>>,
}

impl Connector for Recording {
    fn connect(
        &mut self,
        _network: &NetworkAccess,
        credentials: Credentials,
    ) -> Result<(), CompatError> {
        self.calls
            .lock()
            .expect("calls")
            .push(credentials.lengths());
        Ok(())
    }
}

#[test]
fn credentials_go_to_the_connector_once_and_are_dropped() {
    let dir = tempfile::tempdir().expect("temp dir");
    let acc = acceptance(dir.path());
    let rec = Recording::default();
    let creds = Credentials::new(b"abc", b"defgh").expect("creds");
    let mut b = CompatBackend::with_connector(CompatConfig::new(acc.proof(), creds), rec.clone())
        .expect("backend");
    let before = created_count();
    let now = Instant::now();
    b.submit(now, Command::Connect).expect("connect");
    assert!(!b.holds_credentials());
    assert_eq!(rec.calls.lock().expect("calls").clone(), vec![(3, 5)]);
    assert!(created_count() > before);
    // A second Connect never resends anything (§8.6: no automatic retry).
    b.submit(now, Command::Connect).expect("again");
    assert_eq!(rec.calls.lock().expect("calls").len(), 1);
    let mut out = Vec::new();
    b.poll(now, &mut out);
    assert!(matches!(out.as_slice(), [Event::Disconnected { .. }]));
}

#[test]
fn disconnect_drops_unused_credentials() {
    let dir = tempfile::tempdir().expect("temp dir");
    let acc = acceptance(dir.path());
    let creds = Credentials::new(b"abc", b"defgh").expect("creds");
    let mut b = CompatBackend::new(CompatConfig::new(acc.proof(), creds)).expect("backend");
    b.submit(Instant::now(), Command::Disconnect)
        .expect("disconnect");
    assert!(!b.holds_credentials());
}

#[test]
fn a_proof_for_another_text_builds_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut other = WarningText::shipped();
    other.revision = 99;
    let (acc, write) = consent::accept(
        Some(dir.path()),
        &other,
        UserConfirmation::confirmed_by_user(),
    );
    let _ = write.wait();
    let proof = acc.proof();
    assert!(!proof.is_current());
    assert!(matches!(
        NetworkAccess::new(&proof),
        Err(CompatError::ConsentMismatch)
    ));
    let creds = Credentials::new(b"a", b"b").expect("creds");
    assert!(matches!(
        CompatBackend::new(CompatConfig::new(proof, creds)),
        Err(CompatError::ConsentMismatch)
    ));
}

#[test]
fn config_and_proof_debug_print_no_secret() {
    let dir = tempfile::tempdir().expect("temp dir");
    let acc = acceptance(dir.path());
    let creds = Credentials::new(b"canary-account", b"canary-password").expect("creds");
    let config = CompatConfig::new(acc.proof(), creds);
    let text = format!("{config:?} {:?}", config.proof());
    assert!(!text.contains("canary"), "{text}");
    let b = CompatBackend::new(config).expect("backend");
    assert!(!format!("{b:?}").contains("canary"));
}
