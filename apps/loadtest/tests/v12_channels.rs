// SPDX-License-Identifier: GPL-3.0-or-later
//! Protocol 1.2 against an in-process `tessera-server` (decision 0035, P4 exit criterion 2):
//! two clients exchange tells and join, speak on and leave a named channel while a minor-1
//! peer is kept out of every 1.2 message; and the hostile 1.2 cases of `test-server.md`
//! §13.1 (the client-to-server negative vectors of `tessera-protocol.md` §16.9 and the
//! non-fatal refusals up to the flood rule), after which the server's counts are back at
//! baseline (§13.3).
//!
//! Raw protocol clients (`tessera_loadtest::raw`), no game install. Skips itself (reporting
//! "passed", printing `SKIPPED`) only where loopback UDP is unavailable.

use std::time::Duration;

use tessera_loadtest::raw::{self, ChannelsNeeded, Target, WorldClient};
use tessera_protocol::codes::{AppErrorCode, ChannelRefusedCode, ChatRejectedCode};
use tessera_protocol::message::{
    ChannelLeft, ChannelLeftReason, ChannelRefused, ChatChannel, ChatMessage, ChatSend, Failure,
    ServerCaps,
};
use tessera_protocol::{Message, MessageType};
use tessera_server::accounts::{self, HashParams};
use tessera_server::{Config, Content, DataDir, Server, ServerHandle, ServerOptions};

const PASSWORD: &str = "correct horse battery";
const STEP: Duration = Duration::from_secs(20);

/// Waits until the server's channel and membership counts are `want` (within [`STEP`]).
/// The server stores the counts before it queues the answer that the client has just read,
/// so the first read normally matches; polling keeps the check free of timing assumptions.
async fn wait_counts(handle: &ServerHandle, want: (usize, usize)) {
    let deadline = std::time::Instant::now() + STEP;
    loop {
        let s = handle.stats();
        if (s.channels, s.channel_memberships) == want {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "channel counts {:?}, expected {want:?}",
            (s.channels, s.channel_memberships)
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
/// How long a client waits to be sure a message does not come.
const QUIET: Duration = Duration::from_millis(600);

struct TestServer {
    handle: Option<ServerHandle>,
    dir: std::path::PathBuf,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(h) = self.handle.take() {
            h.shutdown(Duration::ZERO);
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl TestServer {
    fn handle(&self) -> &ServerHandle {
        self.handle.as_ref().expect("server")
    }

    fn target(&self) -> Target {
        let h = self.handle();
        Target {
            addr: h.local_addrs()[0],
            server_name: "127.0.0.1".into(),
            fingerprint: h.fingerprint(),
        }
    }
}

fn temp_dir() -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tessera-loadtest-v12-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn start_server(accounts: &[&str], tune: impl FnOnce(&mut Config)) -> Option<TestServer> {
    if let Err(e) = std::net::UdpSocket::bind("127.0.0.1:0") {
        eprintln!("SKIPPED: no loopback UDP ({e})");
        return None;
    }
    let dir = temp_dir();
    let data = DataDir::new(dir.join("data"));
    let mut config = Config::default();
    config.server.listen = vec!["127.0.0.1:0".parse().unwrap()];
    config.use_hash_floor();
    config.accounts.hash_workers = 2;
    config.accounts.attempts_per_address_per_min = 1_000;
    config.log.stats_interval_s = 0;
    config.server.character_creation = true;
    tune(&mut config);
    data.create().expect("data dir");
    let list: Vec<(&str, &str)> = accounts.iter().map(|a| (*a, PASSWORD)).collect();
    accounts::add_accounts(&data, HashParams::from_config(&config.accounts), &list)
        .expect("accounts");
    std::fs::create_dir_all(data.characters_dir()).expect("characters dir");
    let handle = Server::start(ServerOptions {
        data_dir: data,
        config,
        content: Content::default_world(),
    })
    .expect("the test server starts");
    Some(TestServer {
        handle: Some(handle),
        dir,
    })
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

/// Creates each (account, character) pair.
async fn provision(endpoint: &quinn::Endpoint, target: &Target, chars: &[(&str, &str)]) {
    for (i, (account, character)) in chars.iter().enumerate() {
        let r = raw::provision(
            endpoint,
            target,
            account,
            PASSWORD,
            character,
            false,
            (raw::bot_appearance(i), raw::bot_look(i)),
        )
        .await;
        assert!(r.is_ok(), "{account}: {r:?}");
    }
}

async fn enter(
    endpoint: &quinn::Endpoint,
    target: &Target,
    account: &str,
    character: &str,
    minor: u16,
) -> WorldClient {
    let c = WorldClient::enter(
        endpoint,
        target,
        (account, PASSWORD),
        character,
        minor,
        STEP,
    )
    .await
    .unwrap_or_else(|e| panic!("{account} enters: {e}"));
    assert_eq!(c.minor(), minor);
    c
}

fn chat(channel: ChatChannel, text: &str) -> Message {
    Message::ChatSend(ChatSend {
        channel,
        text: text.into(),
    })
}

fn named(name: &str) -> ChatChannel {
    ChatChannel::Named { name: name.into() }
}

/// The chat line with `text`, as (channel, sender, echo).
fn line_with(text: &str) -> impl FnMut(&Message) -> Option<(ChatChannel, String, bool)> + '_ {
    move |m| match m {
        Message::ChatMessage(ChatMessage {
            channel,
            sender,
            echo,
            text: t,
            ..
        }) if t == text => Some((channel.clone(), sender.clone(), *echo)),
        _ => None,
    }
}

fn joined(m: &Message) -> Option<String> {
    match m {
        Message::ChannelJoined { name } => Some(name.clone()),
        _ => None,
    }
}

fn left(m: &Message) -> Option<ChannelLeft> {
    match m {
        Message::ChannelLeft(l) => Some(l.clone()),
        _ => None,
    }
}

fn refused(m: &Message) -> Option<ChannelRefused> {
    match m {
        Message::ChannelRefused(r) => Some(r.clone()),
        _ => None,
    }
}

/// Nothing of protocol 1.2 is among `seen` (a minor-1 connection, §15 invariant 12; its
/// decoder would have failed on a channel-6 line already).
fn assert_no_v12(seen: &[Message]) {
    for m in seen {
        assert!(
            m.message_type().min_minor() < 2,
            "a 1.2 message reached a minor-1 peer: {m:?}"
        );
        if let Message::ChatMessage(c) = m {
            assert!(!matches!(c.channel, ChatChannel::Named { .. }), "{m:?}");
        }
    }
}

/// P4 exit criterion 2: tells, and channel join, speech and leave on M7's server; a
/// minor-1 peer in the same place sees none of it.
#[test]
fn tells_and_a_named_channel_between_two_clients() {
    let Some(server) = start_server(&["alice", "bob", "carol"], |_| {}) else {
        return;
    };
    let target = server.target();
    let rt = runtime();
    rt.block_on(async {
        let endpoint = raw::endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
        provision(
            &endpoint,
            &target,
            &[("alice", "Aldith"), ("bob", "Bram"), ("carol", "Cora")],
        )
        .await;
        let mut a = enter(&endpoint, &target, "alice", "Aldith", 2).await;
        let mut b = enter(&endpoint, &target, "bob", "Bram", 2).await;
        let mut c = enter(&endpoint, &target, "carol", "Cora", 1).await;
        let mut seen_c = Vec::new();
        let mut skipped = Vec::new();

        // Tells, both ways.
        a.link
            .send(&chat(
                ChatChannel::Tell {
                    peer: "bram".into(),
                },
                "hello Bram",
            ))
            .await
            .unwrap();
        let (ch, from, echo) = b
            .expect_chat(STEP, &mut skipped, line_with("hello Bram"))
            .await
            .unwrap();
        assert_eq!(
            (ch, from.as_str(), echo),
            (
                ChatChannel::Tell {
                    peer: "Aldith".into()
                },
                "Aldith",
                false
            )
        );
        let (ch, _, echo) = a
            .expect_chat(STEP, &mut skipped, line_with("hello Bram"))
            .await
            .unwrap();
        assert_eq!(
            (ch, echo),
            (
                ChatChannel::Tell {
                    peer: "Bram".into()
                },
                true
            )
        );
        b.link
            .send(&chat(
                ChatChannel::Tell {
                    peer: "Aldith".into(),
                },
                "hello Aldith",
            ))
            .await
            .unwrap();
        let (_, from, echo) = a
            .expect_chat(STEP, &mut skipped, line_with("hello Aldith"))
            .await
            .unwrap();
        assert_eq!((from.as_str(), echo), ("Bram", false));

        // Both join one channel; the first join's spelling is the channel's name.
        a.link
            .send(&Message::ChannelJoin {
                name: "Lore Keepers".into(),
            })
            .await
            .unwrap();
        assert_eq!(
            a.expect_chat(STEP, &mut skipped, joined).await.unwrap(),
            "Lore Keepers"
        );
        b.link
            .send(&Message::ChannelJoin {
                name: "lore KEEPERS".into(),
            })
            .await
            .unwrap();
        assert_eq!(
            b.expect_chat(STEP, &mut skipped, joined).await.unwrap(),
            "Lore Keepers"
        );
        wait_counts(server.handle(), (1, 2)).await;

        // A line on the channel reaches both members, the speaker with ECHO.
        a.link
            .send(&chat(named("lore keepers"), "on the channel"))
            .await
            .unwrap();
        for (client, echo) in [(&mut a, true), (&mut b, false)] {
            let (ch, from, e) = client
                .expect_chat(STEP, &mut skipped, line_with("on the channel"))
                .await
                .unwrap();
            assert_eq!(
                (ch, from.as_str(), e),
                (named("Lore Keepers"), "Aldith", echo)
            );
        }
        // Carol (minor 1, same spawn) hears a yell after it, but never the channel line.
        a.link
            .send(&chat(ChatChannel::Yell, "marker one"))
            .await
            .unwrap();
        c.expect_chat(STEP, &mut seen_c, line_with("marker one"))
            .await
            .unwrap();
        assert_no_v12(&seen_c);

        // Bram leaves; Aldith's next line no longer reaches him.
        b.link
            .send(&Message::ChannelLeave {
                name: "Lore Keepers".into(),
            })
            .await
            .unwrap();
        let l = b.expect_chat(STEP, &mut skipped, left).await.unwrap();
        assert_eq!(
            (l.name.as_str(), l.reason),
            ("Lore Keepers", ChannelLeftReason::Left)
        );
        a.link
            .send(&chat(named("Lore Keepers"), "after the leave"))
            .await
            .unwrap();
        a.expect_chat(STEP, &mut skipped, line_with("after the leave"))
            .await
            .unwrap();
        a.link
            .send(&chat(ChatChannel::Yell, "marker two"))
            .await
            .unwrap();
        let mut seen_b = Vec::new();
        b.expect_chat(STEP, &mut seen_b, line_with("marker two"))
            .await
            .unwrap();
        assert!(
            !seen_b
                .iter()
                .any(|m| line_with("after the leave")(m).is_some()),
            "{seen_b:?}"
        );
        // Bram's line on a channel he left is refused.
        b.link
            .send(&chat(named("Lore Keepers"), "let me back"))
            .await
            .unwrap();
        let code = b
            .expect_chat(STEP, &mut skipped, |m| match m {
                Message::ChatRejected(Failure { code, .. }) => Some(*code),
                _ => None,
            })
            .await
            .unwrap();
        assert_eq!(code, ChatRejectedCode::NotJoined);
        c.expect_chat(STEP, &mut seen_c, line_with("marker two"))
            .await
            .unwrap();
        assert_no_v12(&seen_c);
        while let Some(m) = c.next_chat(QUIET).await.unwrap() {
            seen_c.push(m);
        }
        assert_no_v12(&seen_c);
        wait_counts(server.handle(), (1, 1)).await;

        // The last member leaves: the channel is gone.
        a.link
            .send(&Message::ChannelLeave {
                name: "LORE KEEPERS".into(),
            })
            .await
            .unwrap();
        a.expect_chat(STEP, &mut skipped, left).await.unwrap();
        wait_counts(server.handle(), (0, 0)).await;
        for client in [a, b, c] {
            assert!(client.link.conn.close_reason().is_none());
            client.close().await;
        }
    });
}

/// A minor-1 peer's `HelloAck` has no `CHAT_CHANNELS`; a minor-2 one has it unless the
/// operator turned channels off (`test-server.md` §4.6).
#[test]
fn chat_channels_capability_by_minor_and_key() {
    for channels in [true, false] {
        let Some(server) = start_server(&[], |c| c.chat.channels = channels) else {
            return;
        };
        let target = server.target();
        runtime().block_on(async {
            let endpoint = raw::endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
            for minor in [0u16, 1, 2] {
                let conn = raw::connect(&endpoint, &target, STEP).await.unwrap();
                let mut link = raw::ControlLink::open(conn).await.unwrap();
                link.send(&raw::hello_minor(minor)).await.unwrap();
                let caps = link
                    .expect(STEP, |m| match m {
                        Message::HelloAck(a) => Some(a.server_caps),
                        _ => None,
                    })
                    .await
                    .unwrap();
                assert_eq!(
                    caps.contains(ServerCaps::CHAT_CHANNELS),
                    channels && minor == 2,
                    "channels {channels}, minor {minor}"
                );
                link.close().await;
            }
        });
    }
}

/// Sends a channel request carrying exactly `name` and waits for its answer: `None` for
/// `ChannelJoined` or `ChannelLeft`, the code and sanitised name of a `ChannelRefused`.
async fn ask(
    client: &mut WorldClient,
    ty: MessageType,
    name: &[u8],
) -> Option<(ChannelRefusedCode, String)> {
    client
        .link
        .send_raw(&raw::channel_request(ty.code(), name))
        .await
        .unwrap();
    client
        .expect_chat(STEP, &mut Vec::new(), |m| match m {
            Message::ChannelJoined { .. } | Message::ChannelLeft(_) => Some(None),
            Message::ChannelRefused(r) => Some(Some((r.code, r.name.clone()))),
            _ => None,
        })
        .await
        .unwrap()
}

/// Waits for the server to close `conn` and returns its application code.
async fn close_code(conn: &quinn::Connection, what: &str) -> AppErrorCode {
    let closed = tokio::time::timeout(STEP, conn.closed())
        .await
        .unwrap_or_else(|_| panic!("{what}: the server closes the connection"));
    match closed {
        quinn::ConnectionError::ApplicationClosed(a) => {
            let code = u16::try_from(a.error_code.into_inner()).unwrap();
            AppErrorCode::from_u16(code)
        }
        other => panic!("{what}: {other:?}"),
    }
}

/// `test-server.md` §13.1 "Hostile 1.2 cases": each client-to-server negative vector of
/// §16.9, sent in the world, ends the connection with `PROTOCOL_VIOLATION`; the non-fatal
/// cases get their refusal and leave the connection open until the flood rule ends it; the
/// channel and membership counts are back at baseline afterwards (§13.3).
#[test]
fn hostile_v12_cases() {
    let tune = |c: &mut Config| {
        c.chat.max_channels = 2;
        c.chat.max_channels_per_character = 2;
        c.chat.max_channel_members = 2;
    };
    let Some(on) = start_server(&["alice", "bob", "carol"], tune) else {
        return;
    };
    let Some(off) = start_server(&["alice"], |c| c.chat.channels = false) else {
        return;
    };
    let rt = runtime();
    rt.block_on(async {
        let endpoint = raw::endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
        provision(
            &endpoint,
            &on.target(),
            &[("alice", "Aldith"), ("bob", "Bram"), ("carol", "Cora")],
        )
        .await;
        provision(&endpoint, &off.target(), &[("alice", "Aldith")]).await;
        let baseline = on.handle().stats();

        // Fatal: every vector in the world, at its minor.
        for (v, needs) in raw::negative_vectors_v12() {
            let server = if needs == ChannelsNeeded::On {
                &on
            } else {
                &off
            };
            let client = enter(&endpoint, &server.target(), "alice", "Aldith", v.minor_max).await;
            let mut link = client.link;
            link.send_raw(&v.frame).await.unwrap();
            assert_eq!(
                close_code(&link.conn, v.name).await,
                AppErrorCode::ProtocolViolation,
                "{}",
                v.name
            );
        }

        // Non-fatal refusals, each with its code.
        let mut a = enter(&endpoint, &on.target(), "alice", "Aldith", 2).await;
        let mut b = enter(&endpoint, &on.target(), "bob", "Bram", 2).await;
        let mut c = enter(&endpoint, &on.target(), "carol", "Cora", 2).await;
        let mut skipped = Vec::new();
        let join = MessageType::ChannelJoin;
        let leave = MessageType::ChannelLeave;
        let r = |code, name: &str| Some((code, name.to_owned()));
        // The request bucket holds 5 tokens: Aldith's five requests.
        assert_eq!(
            ask(&mut a, join, b"").await,
            r(ChannelRefusedCode::InvalidName, "")
        );
        assert_eq!(
            ask(&mut a, join, b"Lo\x07re").await,
            r(ChannelRefusedCode::InvalidName, "Lo re")
        );
        assert_eq!(
            ask(&mut a, join, b"Lore!").await,
            r(ChannelRefusedCode::InvalidName, "Lore!")
        );
        assert_eq!(
            ask(&mut a, leave, b"Elsewhere").await,
            r(ChannelRefusedCode::NotJoined, "Elsewhere")
        );
        assert_eq!(ask(&mut a, join, b"Lore").await, None);
        // Bram: already joined, per character, server-wide.
        assert_eq!(ask(&mut b, join, b"Lore").await, None);
        assert_eq!(
            ask(&mut b, join, b"lore").await,
            r(ChannelRefusedCode::AlreadyJoined, "lore")
        );
        assert_eq!(ask(&mut b, join, b"Second").await, None);
        assert_eq!(
            ask(&mut b, join, b"Third").await,
            r(ChannelRefusedCode::TooManyChannels, "Third")
        );
        // Cora: the channel is full (2 members), and a third channel is over the limit.
        assert_eq!(
            ask(&mut c, join, b"Lore").await,
            r(ChannelRefusedCode::ChannelFull, "Lore")
        );
        assert_eq!(
            ask(&mut c, join, b"Third").await,
            r(ChannelRefusedCode::ServerLimit, "Third")
        );
        // A line on a channel Cora is not in.
        c.link.send(&chat(named("Lore"), "hi")).await.unwrap();
        let code = c
            .expect_chat(STEP, &mut skipped, |m| match m {
                Message::ChatRejected(f) => Some(f.code),
                _ => None,
            })
            .await
            .unwrap();
        assert_eq!(code, ChatRejectedCode::NotJoined);
        for client in [&a, &b, &c] {
            assert!(client.link.conn.close_reason().is_none());
        }
        // 30 join requests in a burst: Cora has 3 tokens left, then RATE_LIMITED until the
        // flood rule (20 rejections) ends the session.
        for i in 0..30 {
            let _ = c
                .link
                .send_raw(&raw::channel_request(
                    join.code(),
                    format!("Burst{i}").as_bytes(),
                ))
                .await;
        }
        let mut limited = 0;
        while let Ok(Some(m)) = c.next_chat(STEP).await {
            if refused(&m).is_some_and(|r| r.code == ChannelRefusedCode::RateLimited) {
                limited += 1;
            }
        }
        assert!(limited >= 19, "{limited} RATE_LIMITED refusals");
        let mut link = c.link;
        let code = close_code(&link.conn, "burst").await;
        assert_eq!(code, AppErrorCode::Flooding);
        link.finish();
        for client in [a, b] {
            client.close().await;
        }

        // Back at baseline once everyone has left (§13.3).
        let deadline = std::time::Instant::now() + STEP;
        loop {
            let now = on.handle().stats();
            if now.channels == baseline.channels
                && now.channel_memberships == baseline.channel_memberships
                && now.in_world == baseline.in_world
            {
                assert_eq!((now.channels, now.channel_memberships), (0, 0));
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "counts not back at baseline: {now:?}"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    });
}
