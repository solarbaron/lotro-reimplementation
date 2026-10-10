// SPDX-License-Identifier: GPL-3.0-or-later
//! A scripted Tessera-protocol peer for the test-server backend's tests, built only from
//! `tessera-protocol` and quinn over loopback UDP (no `tessera-server`, which would be a
//! dependency cycle). Each test writes the server's side as an async script.

#![allow(dead_code)]

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_net::remote::{FirstUse, Password, RemoteBackend, RemoteConfig, Trust};
use tessera_net::{Event, Session};
use tessera_protocol::frame::{
    FrameDecoder, StreamKind, decode_datagram, encode_datagram, frame_to_vec_minor,
};
use tessera_protocol::message::*;
use tessera_protocol::transport::{ServerIdentity, endpoint_config, server_transport_config};
use tessera_protocol::types::{Appearance, Body, Location, Motion, Pose, Vec3};
use tessera_protocol::value::{Property, Value, pid};
use tessera_protocol::{Channel, Message, Side};
use tokio::sync::mpsc;

pub use tessera_protocol::fingerprint::Fingerprint;

/// How long a peer step or a client wait may take before the test fails.
pub const STEP: Duration = Duration::from_secs(10);

/// A running scripted peer.
pub struct PeerServer<T> {
    pub addr: SocketAddr,
    pub fingerprint: Fingerprint,
    handle: std::thread::JoinHandle<T>,
}

impl<T> PeerServer<T> {
    /// Waits for the script's result.
    pub fn join(self) -> T {
        self.handle.join().expect("peer script panicked")
    }

    /// A connect string for this peer, with or without the pin.
    pub fn connect_string(&self, pin: bool) -> String {
        if pin {
            format!("tessera://{}?pin={}", self.addr, self.fingerprint)
        } else {
            format!("tessera://{}", self.addr)
        }
    }

    /// A pinned client configuration for account `alice`.
    pub fn config(&self) -> RemoteConfig {
        RemoteConfig::new(
            &self.connect_string(true),
            "alice",
            Password::new("correct horse"),
        )
        .unwrap()
    }
}

/// Options for [`spawn`].
#[derive(Clone)]
pub struct PeerOptions {
    pub identity: Option<Arc<ServerIdentity>>,
    pub alpn: Option<Vec<u8>>,
    /// Answer `TimeRequest`s (server clock = peer start + 10 s).
    pub time_replies: bool,
}

impl Default for PeerOptions {
    fn default() -> Self {
        Self {
            identity: None,
            alpn: None,
            time_replies: true,
        }
    }
}

/// Starts a peer that accepts one connection and runs `script` on it.
pub fn spawn<T, F, Fut>(options: PeerOptions, script: F) -> PeerServer<T>
where
    T: Send + 'static,
    F: FnOnce(Peer) -> Fut + Send + 'static,
    Fut: Future<Output = T>,
{
    let identity = options
        .identity
        .clone()
        .unwrap_or_else(|| Arc::new(ServerIdentity::generate().unwrap()));
    let fingerprint = identity.fingerprint();
    let (addr_tx, addr_rx) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            let mut rustls = identity.rustls_server_config().unwrap();
            if let Some(alpn) = &options.alpn {
                rustls.alpn_protocols = vec![alpn.clone()];
            }
            let crypto = quinn::crypto::rustls::QuicServerConfig::try_from(rustls).unwrap();
            let mut server = quinn::ServerConfig::with_crypto(Arc::new(crypto));
            server.transport_config(Arc::new(server_transport_config()));
            let socket = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
            let addr = socket.local_addr().unwrap();
            let endpoint = quinn::Endpoint::new(
                endpoint_config(),
                Some(server),
                socket,
                Arc::new(quinn::TokioRuntime),
            )
            .unwrap();
            addr_tx.send(addr).unwrap();
            let incoming = tokio::time::timeout(STEP, endpoint.accept())
                .await
                .expect("no client connected")
                .expect("endpoint closed");
            let conn = match incoming.await {
                Ok(c) => c,
                Err(e) => {
                    // A handshake the client refused: run the script without a connection.
                    return script(Peer::failed(e.to_string())).await;
                }
            };
            let peer = Peer::new(conn, options.time_replies);
            let out = script(peer).await;
            // Give the client time to see whatever the script sent last.
            let _ = tokio::time::timeout(Duration::from_secs(3), endpoint.wait_idle()).await;
            out
        })
    });
    let addr = addr_rx.recv_timeout(STEP).expect("peer did not start");
    PeerServer {
        addr,
        fingerprint,
        handle,
    }
}

/// The server's side of one connection.
pub struct Peer {
    pub conn: Option<quinn::Connection>,
    pub handshake_error: Option<String>,
    ctl: Option<(quinn::SendStream, quinn::RecvStream)>,
    decoder: FrameDecoder,
    inbuf: Vec<u8>,
    world: Option<quinn::SendStream>,
    chat: Option<quinn::SendStream>,
    datagrams: Option<mpsc::Receiver<Message>>,
    start: Instant,
    /// The negotiated minor the peer encodes and decodes under (0 unless set).
    minor: u16,
}

impl Peer {
    fn new(conn: quinn::Connection, time_replies: bool) -> Self {
        let start = Instant::now();
        let (tx, rx) = mpsc::channel(4096);
        let c = conn.clone();
        tokio::spawn(async move {
            while let Ok(d) = c.read_datagram().await {
                let Ok(m) = decode_datagram(Side::Client, &d) else {
                    continue;
                };
                if let Message::TimeRequest { client_time } = m {
                    if time_replies {
                        let t = server_time_at(start);
                        let reply = Message::TimeReply(tessera_protocol::time::time_reply(
                            client_time,
                            t,
                            t,
                            30,
                        ));
                        let _ = c.send_datagram(encode_datagram(&reply).unwrap().into());
                    }
                    continue;
                }
                if tx.try_send(m).is_err() {
                    // The script does not read datagrams; drop.
                }
            }
        });
        Self {
            conn: Some(conn),
            handshake_error: None,
            ctl: None,
            decoder: FrameDecoder::new(Channel::Control, Side::Client),
            inbuf: Vec::new(),
            world: None,
            chat: None,
            datagrams: Some(rx),
            start,
            minor: 0,
        }
    }

    fn failed(e: String) -> Self {
        Self {
            conn: None,
            handshake_error: Some(e),
            ctl: None,
            decoder: FrameDecoder::new(Channel::Control, Side::Client),
            inbuf: Vec::new(),
            world: None,
            chat: None,
            datagrams: None,
            start: Instant::now(),
            minor: 0,
        }
    }

    pub fn conn(&self) -> &quinn::Connection {
        self.conn.as_ref().expect("no connection")
    }

    /// Sets the negotiated minor version for every frame after the `HelloAck`.
    pub fn set_minor(&mut self, minor: u16) {
        self.minor = minor;
        self.decoder.set_minor(minor);
    }

    /// The peer's server clock, µs.
    pub fn now(&self) -> u64 {
        server_time_at(self.start)
    }

    async fn control(&mut self) -> &mut (quinn::SendStream, quinn::RecvStream) {
        if self.ctl.is_none() {
            let s = tokio::time::timeout(STEP, self.conn().accept_bi())
                .await
                .expect("no control stream")
                .expect("connection lost");
            self.ctl = Some(s);
        }
        self.ctl.as_mut().unwrap()
    }

    /// The next control message from the client other than `Ping` (answered with `Pong`
    /// here), or `None` when the stream or connection ended.
    pub async fn recv(&mut self) -> Option<Message> {
        loop {
            match self.recv_any().await? {
                Message::Ping { nonce } => self.send(Message::Pong { nonce }).await,
                m => return Some(m),
            }
        }
    }

    /// The next control message from the client, pings included.
    pub async fn recv_any(&mut self) -> Option<Message> {
        loop {
            let mut input: &[u8] = &self.inbuf;
            let r = self.decoder.push(&mut input);
            let used = self.inbuf.len() - input.len();
            self.inbuf.drain(..used);
            match r {
                Ok(Some(m)) => return Some(m),
                Ok(None) => {}
                Err(e) => panic!("client sent a bad frame: {e}"),
            }
            let mut buf = [0u8; 4096];
            let (_, recv) = self.control().await;
            match tokio::time::timeout(STEP, recv.read(&mut buf)).await {
                Ok(Ok(Some(n))) => self.inbuf.extend_from_slice(&buf[..n]),
                _ => return None,
            }
        }
    }

    /// The next control message, which must be of the given type.
    pub async fn expect(&mut self, ty: MessageType) -> Message {
        let m = self
            .recv()
            .await
            .unwrap_or_else(|| panic!("expected {ty}, got nothing"));
        assert_eq!(m.message_type(), ty, "unexpected message {m:?}");
        m
    }

    pub async fn send(&mut self, m: Message) {
        let bytes = frame_to_vec_minor(&m, self.minor).unwrap();
        self.send_raw(&bytes).await;
    }

    pub async fn send_raw(&mut self, bytes: &[u8]) {
        let (send, _) = self.control().await;
        send.write_all(bytes).await.expect("control write");
    }

    /// Finishes (FIN) the server's side of the control stream.
    pub fn finish_control(&mut self) {
        if let Some((send, _)) = self.ctl.as_mut() {
            let _ = send.finish();
        }
    }

    pub async fn chat_raw(&mut self, bytes: &[u8]) {
        self.chat
            .as_mut()
            .expect("chat stream")
            .write_all(bytes)
            .await
            .expect("chat write");
    }

    pub async fn open(&mut self, kind: StreamKind) {
        let mut s = self.conn().open_uni().await.unwrap();
        s.write_all(&[kind.to_byte()]).await.unwrap();
        match kind {
            StreamKind::World => self.world = Some(s),
            StreamKind::Chat => self.chat = Some(s),
        }
    }

    pub async fn open_raw(&mut self, kind_byte: u8) -> quinn::SendStream {
        let mut s = self.conn().open_uni().await.unwrap();
        s.write_all(&[kind_byte]).await.unwrap();
        s
    }

    pub async fn world(&mut self, m: Message) {
        let bytes = frame_to_vec_minor(&m, self.minor).unwrap();
        self.world_raw(&bytes).await;
    }

    pub async fn world_raw(&mut self, bytes: &[u8]) {
        self.world
            .as_mut()
            .expect("world stream")
            .write_all(bytes)
            .await
            .expect("world write");
    }

    pub async fn chat(&mut self, m: Message) {
        let bytes = frame_to_vec_minor(&m, self.minor).unwrap();
        self.chat
            .as_mut()
            .expect("chat stream")
            .write_all(&bytes)
            .await
            .expect("chat write");
    }

    pub fn datagram(&self, m: &Message) {
        let bytes = encode_datagram(m).unwrap();
        self.datagram_raw(&bytes);
    }

    pub fn datagram_raw(&self, bytes: &[u8]) {
        let _ = self.conn().send_datagram(bytes.to_vec().into());
    }

    /// The next non-time datagram from the client.
    pub async fn recv_datagram(&mut self) -> Option<Message> {
        let rx = self.datagrams.as_mut()?;
        tokio::time::timeout(STEP, rx.recv()).await.ok().flatten()
    }

    /// Waits for a client datagram matching `pred`.
    pub async fn datagram_where(&mut self, mut pred: impl FnMut(&Message) -> bool) -> Message {
        loop {
            let m = self.recv_datagram().await.expect("no matching datagram");
            if pred(&m) {
                return m;
            }
        }
    }

    /// Hello → HelloAck, Login → LoginOk with `characters`, then the chat stream.
    pub async fn login(&mut self, characters: &[&str]) {
        self.expect(MessageType::Hello).await;
        self.send(hello_ack()).await;
        let login = self.expect(MessageType::Login).await;
        if let Message::Login(c) = login {
            assert_eq!(c.account, "alice");
            assert_eq!(c.password, "correct horse");
        }
        self.send(Message::LoginOk(LoginOk {
            account: "alice".into(),
            characters: characters
                .iter()
                .map(|n| tessera_protocol::types::CharacterSummary {
                    name: (*n).into(),
                    body: Body::Character {
                        species: 23,
                        sex: 1,
                    },
                })
                .collect(),
            looks: Vec::new(),
        }))
        .await;
        self.open(StreamKind::Chat).await;
    }

    /// EnterWorld → world stream with WorldEnter (player 42 at [`start_location`]) and
    /// WorldClock.
    pub async fn enter_world(&mut self) {
        let m = self.expect(MessageType::EnterWorld).await;
        if let Message::EnterWorld(e) = m {
            assert_eq!(e.name, "Aldith");
        }
        self.open(StreamKind::World).await;
        let now = self.now();
        self.world(Message::WorldEnter(WorldEnter {
            player: 42,
            name: "Aldith".into(),
            instance: 1,
            epoch: 1,
            server_time: now,
            pose: Pose {
                location: start_location(),
                heading: 0.0,
                anchor: None,
            },
            appearance: appearance(),
            look: tessera_protocol::types::Look::none(),
        }))
        .await;
        self.world(Message::WorldClock(WorldClock {
            server_time: now,
            game_clock: 43_200.0,
            rate: 1.0,
        }))
        .await;
    }

    /// Keeps the connection and its streams open until the client closes it (dropping a
    /// receive stream would send STOP_SENDING, which the client rightly treats as a
    /// violation).
    pub async fn linger(self) {
        if let Some(c) = &self.conn {
            let _ = tokio::time::timeout(STEP, c.closed()).await;
        }
    }

    /// Waits until the client closes the connection; returns how.
    pub async fn closed(&self) -> quinn::ConnectionError {
        tokio::time::timeout(STEP, self.conn().closed())
            .await
            .expect("client did not close")
    }
}

fn server_time_at(start: Instant) -> u64 {
    10_000_000 + u64::try_from(start.elapsed().as_micros()).unwrap()
}

pub fn hello_ack() -> Message {
    Message::HelloAck(HelloAck {
        minor: 0,
        server_caps: ServerCaps::APPEARANCE_CHANGE,
        tick_rate: 30,
        snapshot_rate: 15,
        interest_radius: 2,
        server_name: "Test Shard".into(),
        motd: String::new(),
    })
}

pub fn start_location() -> Location {
    Location::new(1, 20_250.5, 22_500.25, 401.0)
}

pub fn appearance() -> Appearance {
    Appearance::new(Body::Character {
        species: 23,
        sex: 1,
    })
}

pub fn entity_create(id: u64, server_time: u64, x: f64, vx: f32) -> Message {
    Message::EntityCreate(EntityCreate {
        id,
        kind: EntityKind::Player,
        name: "Bram".into(),
        class: None,
        server_time,
        pose: Pose {
            location: Location::new(1, x, 22_500.0, 400.5),
            heading: 0.0,
            anchor: None,
        },
        velocity: Vec3 {
            x: vx,
            y: 0.0,
            z: 0.0,
        },
        motion: Motion::STANDING,
        appearance: Appearance::new(Body::Character {
            species: 81,
            sex: 2,
        }),
        properties: vec![Property::new(pid::STANCE, Value::Enum(1))],
        look: tessera_protocol::types::Look::none(),
    })
}

pub fn motion(server_time: u64, id: u64, x: f32, vx: f32) -> Message {
    Message::MotionBatch(MotionBatch {
        server_time,
        entries: vec![MotionEntry {
            id,
            position: Vec3 {
                x,
                y: 22_500.0,
                z: 400.5,
            },
            heading: 0.0,
            velocity: Vec3 {
                x: vx,
                y: 0.0,
                z: 0.0,
            },
            motion: Motion::new(tessera_protocol::types::Gait::Walking, false),
        }],
    })
}

/// A session over a backend for `config`, already sent `Connect`.
pub fn connect(config: RemoteConfig) -> Session {
    let mut s = Session::new(RemoteBackend::new(config).unwrap());
    s.send(Instant::now(), tessera_net::Command::Connect)
        .unwrap();
    s
}

thread_local! {
    /// Events polled after the one a [`pump`] stopped at, kept for the next call.
    static BACKLOG: std::cell::RefCell<std::collections::VecDeque<Event>> =
        const { std::cell::RefCell::new(std::collections::VecDeque::new()) };
}

/// Polls `session` until an event matches `pred` (returns every event seen, the match
/// last; later events are kept for the next call). Panics after [`STEP`].
pub fn pump(session: &mut Session, mut pred: impl FnMut(&Event) -> bool) -> Vec<Event> {
    let deadline = Instant::now() + STEP;
    let mut seen = Vec::new();
    let mut buf = Vec::new();
    loop {
        let mut batch: std::collections::VecDeque<Event> =
            BACKLOG.with(|b| std::mem::take(&mut *b.borrow_mut()));
        session.poll_into(Instant::now(), &mut buf);
        batch.extend(buf.drain(..));
        while let Some(e) = batch.pop_front() {
            let hit = pred(&e);
            seen.push(e);
            if hit {
                BACKLOG.with(|b| b.borrow_mut().extend(batch));
                return seen;
            }
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting; events so far: {seen:#?}"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Takes the events a [`pump`] kept after its match.
pub fn backlog() -> Vec<Event> {
    BACKLOG.with(|b| b.borrow_mut().drain(..).collect())
}

/// Polls for `d`, returning every event (including any kept by [`pump`]).
pub fn pump_for(session: &mut Session, d: Duration) -> Vec<Event> {
    let deadline = Instant::now() + d;
    let mut seen: Vec<Event> = BACKLOG.with(|b| b.borrow_mut().drain(..).collect());
    while Instant::now() < deadline {
        session.poll_into(Instant::now(), &mut seen);
        std::thread::sleep(Duration::from_millis(2));
    }
    seen
}

/// The backend inside `session`.
pub fn backend(session: &Session) -> &RemoteBackend {
    session.backend::<RemoteBackend>().unwrap()
}

/// A trust configuration with a known-servers file.
pub fn trust(path: std::path::PathBuf, first_use: FirstUse) -> Trust {
    Trust {
        known_servers: Some(path),
        first_use,
    }
}

/// A fresh temporary directory for one test.
pub fn temp_dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("tessera-net-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}
