// SPDX-License-Identifier: GPL-3.0-or-later
//! In-process test servers and raw protocol clients built only from `tessera-protocol` and
//! quinn over loopback UDP.

#![allow(dead_code)]

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tessera_protocol::codes::AppErrorCode;
use tessera_protocol::connect::ExpectedKey;
use tessera_protocol::frame::{
    FrameDecoder, StreamKind, decode_datagram, encode_datagram, frame_to_vec,
};
use tessera_protocol::message::*;
use tessera_protocol::transport::{PinnedServerVerifier, client_endpoint, quinn_client_config};
use tessera_protocol::types::{Appearance, Body, Location, Motion, Vec3};
use tessera_protocol::{Channel, Message, Side};
use tessera_server::accounts::{self, HashParams};
use tessera_server::{Config, Content, DataDir, Server, ServerHandle, ServerOptions};
use tokio::sync::mpsc;

/// Longest wait for one expected message.
pub const WAIT: Duration = Duration::from_secs(10);
/// The password of every test account.
pub const PASSWORD: &str = "correct horse battery";

/// A server in a temporary data directory.
pub struct TestServer {
    pub handle: Option<ServerHandle>,
    pub dir: tempfile::TempDir,
}

impl TestServer {
    /// A test configuration: loopback, port 0, the Argon2id floor, generous login budget.
    pub fn config() -> Config {
        let mut c = Config::default();
        c.server.listen = vec!["127.0.0.1:0".parse().unwrap()];
        c.use_hash_floor();
        c.accounts.hash_workers = 2;
        c.accounts.attempts_per_address_per_min = 1_000;
        c.world.teleport_interval_ms = 0;
        c.log.stats_interval_s = 0;
        c
    }

    /// Starts a server with `accounts` created beforehand.
    pub fn start(config: Config, content: Content, accounts_list: &[&str]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let data = DataDir::new(dir.path().join("data"));
        Self::create_accounts(&data, &config, accounts_list);
        let handle = Server::start(ServerOptions {
            data_dir: data,
            config,
            content,
        })
        .expect("server starts");
        Self {
            handle: Some(handle),
            dir,
        }
    }

    /// Restarts on the same data directory.
    pub fn restart(&mut self, config: Config, content: Content) {
        if let Some(h) = self.handle.take() {
            h.shutdown(Duration::ZERO);
        }
        let handle = Server::start(ServerOptions {
            data_dir: self.data_dir(),
            config,
            content,
        })
        .expect("server restarts");
        self.handle = Some(handle);
    }

    pub fn create_accounts(data: &DataDir, config: &Config, names: &[&str]) {
        data.create().unwrap();
        let entries: Vec<(&str, &str)> = names.iter().map(|n| (*n, PASSWORD)).collect();
        accounts::add_accounts(data, HashParams::from_config(&config.accounts), &entries).unwrap();
    }

    pub fn data_dir(&self) -> DataDir {
        DataDir::new(self.dir.path().join("data"))
    }

    pub fn handle(&self) -> &ServerHandle {
        self.handle.as_ref().unwrap()
    }

    pub fn addr(&self) -> SocketAddr {
        self.handle().local_addr()
    }

    pub async fn client(&self) -> RawClient {
        RawClient::connect(self.addr(), self.handle().fingerprint()).await
    }

    /// A client that logged in and entered the world with a new character.
    pub async fn player(&self, account: &str, character: &str) -> (RawClient, WorldEnter) {
        let mut c = self.client().await;
        c.hello().await;
        c.login(account, PASSWORD).await.expect("login ok");
        c.create(character, appearance(23)).await.expect("created");
        let w = c.enter(character).await.expect("entered");
        (c, w)
    }

    pub fn shutdown(&mut self) -> tessera_server::ShutdownReport {
        self.handle.take().unwrap().shutdown(Duration::ZERO)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(h) = self.handle.take() {
            h.shutdown(Duration::ZERO);
        }
    }
}

/// A test appearance (invented numbers).
pub fn appearance(species: u32) -> Appearance {
    Appearance::new(Body::Character { species, sex: 1 })
}

/// What the client received.
#[derive(Debug, Clone)]
pub enum Received {
    Control(Message),
    World(Message),
    Chat(Message),
    Datagram(Message),
    /// A decoding error (the server broke the protocol).
    Error(String),
    /// The connection closed: code and reason.
    Closed(Option<u64>, String),
}

/// A raw protocol client.
pub struct RawClient {
    pub endpoint: quinn::Endpoint,
    pub conn: quinn::Connection,
    pub send: quinn::SendStream,
    rx: mpsc::UnboundedReceiver<Received>,
    pub buffer: VecDeque<Received>,
    pub closed: Option<(Option<u64>, String)>,
    seq: u32,
    pub epoch: u32,
    /// Frame and datagram bytes this client handed to QUIC.
    pub sent_bytes: Arc<AtomicU64>,
    /// Frame and datagram bytes this client read from QUIC (stream-kind bytes excluded).
    pub received_bytes: Arc<AtomicU64>,
}

impl RawClient {
    pub async fn connect(
        addr: SocketAddr,
        pin: tessera_protocol::fingerprint::Fingerprint,
    ) -> Self {
        let endpoint = client_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
        let verifier = PinnedServerVerifier::new(ExpectedKey::Pinned(pin), None);
        let cfg = quinn_client_config(verifier).unwrap();
        let conn = endpoint
            .connect_with(cfg, addr, "127.0.0.1")
            .unwrap()
            .await
            .expect("QUIC handshake");
        let (send, recv) = conn.open_bi().await.unwrap();
        let (tx, rx) = mpsc::unbounded_channel();
        let sent_bytes = Arc::new(AtomicU64::new(0));
        let received_bytes = Arc::new(AtomicU64::new(0));
        // Control stream reader.
        let t = tx.clone();
        let count = Arc::clone(&received_bytes);
        tokio::spawn(async move {
            let mut recv = recv;
            let mut dec = FrameDecoder::new(Channel::Control, Side::Server);
            let mut buf = vec![0u8; 16 * 1024];
            loop {
                match recv.read(&mut buf).await {
                    Ok(Some(n)) => {
                        count.fetch_add(n as u64, Ordering::Relaxed);
                        let mut input: &[u8] = &buf[..n];
                        loop {
                            match dec.push(&mut input) {
                                Ok(Some(m)) => {
                                    let _ = t.send(Received::Control(m));
                                }
                                Ok(None) => break,
                                Err(e) => {
                                    let _ = t.send(Received::Error(e.to_string()));
                                    return;
                                }
                            }
                        }
                    }
                    _ => return,
                }
            }
        });
        // Unidirectional streams.
        let c = conn.clone();
        let t = tx.clone();
        let count = Arc::clone(&received_bytes);
        tokio::spawn(async move {
            while let Ok(mut s) = c.accept_uni().await {
                let t = t.clone();
                let count = Arc::clone(&count);
                tokio::spawn(async move {
                    let mut kind = [0u8; 1];
                    if s.read_exact(&mut kind).await.is_err() {
                        return;
                    }
                    let Ok(kind) = StreamKind::from_byte(kind[0]) else {
                        let _ = t.send(Received::Error("bad stream kind".into()));
                        return;
                    };
                    let mut dec = FrameDecoder::new(kind.channel(), Side::Server);
                    let mut buf = vec![0u8; 64 * 1024];
                    loop {
                        match s.read(&mut buf).await {
                            Ok(Some(n)) => {
                                count.fetch_add(n as u64, Ordering::Relaxed);
                                let mut input: &[u8] = &buf[..n];
                                loop {
                                    match dec.push(&mut input) {
                                        Ok(Some(m)) => {
                                            let r = match kind {
                                                StreamKind::World => Received::World(m),
                                                StreamKind::Chat => Received::Chat(m),
                                            };
                                            let _ = t.send(r);
                                        }
                                        Ok(None) => break,
                                        Err(e) => {
                                            let _ = t.send(Received::Error(e.to_string()));
                                            return;
                                        }
                                    }
                                }
                            }
                            _ => return,
                        }
                    }
                });
            }
        });
        // Datagrams.
        let c = conn.clone();
        let t = tx.clone();
        let count = Arc::clone(&received_bytes);
        tokio::spawn(async move {
            while let Ok(d) = c.read_datagram().await {
                count.fetch_add(d.len() as u64, Ordering::Relaxed);
                match decode_datagram(Side::Server, &d) {
                    Ok(m) => {
                        let _ = t.send(Received::Datagram(m));
                    }
                    Err(e) => {
                        let _ = t.send(Received::Error(e.to_string()));
                    }
                }
            }
        });
        // Close watcher.
        let c = conn.clone();
        tokio::spawn(async move {
            let e = c.closed().await;
            let (code, reason) = match &e {
                quinn::ConnectionError::ApplicationClosed(a) => (
                    Some(a.error_code.into_inner()),
                    String::from_utf8_lossy(&a.reason).into_owned(),
                ),
                other => (None, other.to_string()),
            };
            let _ = tx.send(Received::Closed(code, reason));
        });
        Self {
            endpoint,
            conn,
            send,
            rx,
            buffer: VecDeque::new(),
            closed: None,
            seq: 0,
            epoch: 1,
            sent_bytes,
            received_bytes,
        }
    }

    pub async fn send(&mut self, m: &Message) {
        let b = frame_to_vec(m).unwrap();
        self.send.write_all(&b).await.unwrap();
        self.sent_bytes.fetch_add(b.len() as u64, Ordering::Relaxed);
    }

    pub async fn send_raw(&mut self, b: &[u8]) {
        if self.send.write_all(b).await.is_ok() {
            self.sent_bytes.fetch_add(b.len() as u64, Ordering::Relaxed);
        }
    }

    pub fn datagram(&self, m: &Message) {
        let b = encode_datagram(m).unwrap();
        let n = b.len() as u64;
        if self.conn.send_datagram(b.into()).is_ok() {
            self.sent_bytes.fetch_add(n, Ordering::Relaxed);
        }
    }

    pub fn datagram_raw(&self, b: &[u8]) {
        if self.conn.send_datagram(b.to_vec().into()).is_ok() {
            self.sent_bytes.fetch_add(b.len() as u64, Ordering::Relaxed);
        }
    }

    /// Bytes this client sent and received so far (frames and datagrams).
    pub fn byte_counts(&self) -> (u64, u64) {
        (
            self.sent_bytes.load(Ordering::Relaxed),
            self.received_bytes.load(Ordering::Relaxed),
        )
    }

    /// Waits for a received item matching `pred`, keeping the others buffered.
    pub async fn wait_for<T>(
        &mut self,
        timeout: Duration,
        mut pred: impl FnMut(&Received) -> Option<T>,
    ) -> Option<T> {
        if let Some(i) = self.buffer.iter().position(|r| pred(r).is_some()) {
            let r = self.buffer.remove(i).unwrap();
            return pred(&r);
        }
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let r = tokio::time::timeout_at(deadline, self.rx.recv())
                .await
                .ok()??;
            if let Received::Closed(c, reason) = &r {
                self.closed = Some((*c, reason.clone()));
            }
            if let Some(t) = pred(&r) {
                return Some(t);
            }
            self.buffer.push_back(r);
            if self.buffer.len() > 100_000 {
                self.buffer.pop_front();
            }
        }
    }

    /// Drains everything received so far (for tests that count).
    pub async fn drain(&mut self, settle: Duration) -> Vec<Received> {
        let deadline = tokio::time::Instant::now() + settle;
        while let Ok(Some(r)) = tokio::time::timeout_at(deadline, self.rx.recv()).await {
            if let Received::Closed(c, reason) = &r {
                self.closed = Some((*c, reason.clone()));
            }
            self.buffer.push_back(r);
        }
        self.buffer.drain(..).collect()
    }

    pub async fn control(&mut self) -> Option<Message> {
        self.wait_for(WAIT, |r| match r {
            Received::Control(m) => Some(m.clone()),
            _ => None,
        })
        .await
    }

    pub async fn world_msg<T>(&mut self, mut f: impl FnMut(&Message) -> Option<T>) -> Option<T> {
        self.wait_for(WAIT, |r| match r {
            Received::World(m) => f(m),
            _ => None,
        })
        .await
    }

    pub async fn chat_msg<T>(&mut self, mut f: impl FnMut(&Message) -> Option<T>) -> Option<T> {
        self.wait_for(WAIT, |r| match r {
            Received::Chat(m) => f(m),
            _ => None,
        })
        .await
    }

    /// Waits for the connection to close; returns the application code.
    pub async fn closed_code(&mut self, timeout: Duration) -> Option<u64> {
        if let Some((c, _)) = &self.closed {
            return *c;
        }
        self.wait_for(timeout, |r| match r {
            Received::Closed(c, _) => Some(*c),
            _ => None,
        })
        .await
        .flatten()
    }

    /// Waits for a `Disconnect` on the control stream; returns its code.
    pub async fn disconnect_code(&mut self, timeout: Duration) -> Option<AppErrorCode> {
        self.wait_for(timeout, |r| match r {
            Received::Control(Message::Disconnect(d)) => Some(d.code),
            _ => None,
        })
        .await
    }

    pub async fn hello(&mut self) -> HelloAck {
        self.send(&Message::Hello(Hello {
            minor_max: 0,
            client_caps: 0,
            max_entities: 1024,
            client_name: "tessera-server tests".into(),
        }))
        .await;
        match self.control().await {
            Some(Message::HelloAck(a)) => a,
            other => panic!("expected HelloAck, got {other:?}"),
        }
    }

    pub async fn login(&mut self, account: &str, password: &str) -> Result<LoginOk, LoginFailed> {
        self.send(&Message::Login(Credentials {
            account: account.into(),
            password: password.into(),
        }))
        .await;
        loop {
            match self.control().await {
                Some(Message::LoginOk(ok)) => return Ok(ok),
                Some(Message::LoginFailed(f)) => return Err(f),
                Some(Message::Pong { .. }) => {}
                other => panic!("expected a login answer, got {other:?}"),
            }
        }
    }

    pub async fn create(&mut self, name: &str, a: Appearance) -> Result<CharacterList, String> {
        self.send(&Message::CharacterCreate(CharacterCreate {
            name: name.into(),
            appearance: a,
            look: tessera_protocol::types::Look::none(),
        }))
        .await;
        match self.control().await {
            Some(Message::CharacterList(l)) => Ok(l),
            Some(Message::CharacterCreateFailed(f)) => Err(f.code.name().to_owned()),
            other => Err(format!("{other:?}")),
        }
    }

    pub async fn enter(&mut self, name: &str) -> Result<WorldEnter, String> {
        self.send(&Message::EnterWorld(EnterWorld { name: name.into() }))
            .await;
        let r = self
            .wait_for(WAIT, |r| match r {
                Received::World(Message::WorldEnter(w)) => Some(Ok(w.clone())),
                Received::Control(Message::EnterWorldFailed(f)) => {
                    Some(Err(f.code.name().to_owned()))
                }
                _ => None,
            })
            .await
            .unwrap_or_else(|| Err("timeout".into()))?;
        self.epoch = r.epoch;
        Ok(r)
    }

    /// Sends a `MoveInput` at `location` with the current epoch.
    pub fn move_to(&mut self, location: Location, velocity: Vec3, motion: Motion) {
        self.seq = self.seq.wrapping_add(1);
        self.datagram(&Message::MoveInput(MoveInput {
            seq: self.seq,
            epoch: self.epoch,
            location,
            heading: 0.0,
            velocity,
            motion,
        }));
    }

    /// Waits for an `EntityCreate` of `kind`.
    pub async fn created(&mut self, kind: EntityKind) -> Option<EntityCreate> {
        self.world_msg(|m| match m {
            Message::EntityCreate(c) if c.kind == kind => Some(c.clone()),
            _ => None,
        })
        .await
    }

    pub async fn chat(&mut self, channel: ChatChannel, text: &str) {
        self.send(&Message::ChatSend(ChatSend {
            channel,
            text: text.into(),
        }))
        .await;
    }
}
