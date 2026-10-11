// SPDX-License-Identifier: GPL-3.0-or-later
//! Raw protocol clients built from `tessera-protocol` and quinn, for the two jobs the
//! neutral [`tessera_net::Session`] has no command for: creating the bots' characters
//! (`CharacterCreate`, `tessera-protocol.md` §6.2.7) and the hostile mode, which must send
//! bytes no well-behaved client would.
//!
//! Every wait is bounded by a timeout, received data is decoded with the protocol's own
//! decoders, and the amount buffered per connection is bounded.

use std::net::SocketAddr;
use std::time::Duration;

use tessera_protocol::codes::{AppErrorCode, LoginFailedCode};
use tessera_protocol::connect::ExpectedKey;
use tessera_protocol::fingerprint::Fingerprint;
use tessera_protocol::frame::{FrameDecoder, StreamKind, frame_to_vec_minor};
use tessera_protocol::message::{CharacterCreate, Credentials, Disconnect, EnterWorld, Hello};
use tessera_protocol::transport::{PinnedServerVerifier, client_endpoint, quinn_client_config};
use tessera_protocol::types::{Appearance, Body, Equipped, Look, LookEntry, LookValue};
use tessera_protocol::{Channel, Message, Side};

/// Most undecoded control-stream bytes one raw client keeps (one maximal frame).
pub const MAX_PENDING: usize = tessera_protocol::limits::MAX_FRAME_BYTES;

/// Where the raw clients connect.
#[derive(Debug, Clone)]
pub struct Target {
    /// The server's UDP address.
    pub addr: SocketAddr,
    /// The name passed to TLS (the connect string's host; an IP literal sends no SNI).
    pub server_name: String,
    /// The pinned server key.
    pub fingerprint: Fingerprint,
}

/// A client endpoint bound to `bind` (port 0 for any). Must be called inside a tokio
/// runtime.
///
/// # Errors
///
/// The transport error, in words.
pub fn endpoint(bind: SocketAddr) -> Result<quinn::Endpoint, String> {
    client_endpoint(bind).map_err(|e| e.to_string())
}

/// Opens a QUIC connection with the pinned key, within `timeout`.
///
/// # Errors
///
/// The connection error, in words.
pub async fn connect(
    endpoint: &quinn::Endpoint,
    target: &Target,
    timeout: Duration,
) -> Result<quinn::Connection, String> {
    let verifier = PinnedServerVerifier::new(ExpectedKey::Pinned(target.fingerprint), None);
    let cfg = quinn_client_config(verifier).map_err(|e| e.to_string())?;
    let connecting = endpoint
        .connect_with(cfg, target.addr, &target.server_name)
        .map_err(|e| e.to_string())?;
    match tokio::time::timeout(timeout, connecting).await {
        Ok(Ok(c)) => Ok(c),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err("QUIC handshake timed out".into()),
    }
}

/// The client's control stream on one connection.
#[derive(Debug)]
pub struct ControlLink {
    /// The connection.
    pub conn: quinn::Connection,
    send: quinn::SendStream,
    recv: quinn::RecvStream,
    decoder: FrameDecoder,
    pending: Vec<u8>,
    /// The negotiated minor version, from the server's `HelloAck` (0 before it).
    minor: u16,
}

impl ControlLink {
    /// Opens the control stream (the client's only bidirectional stream, §2.3).
    ///
    /// # Errors
    ///
    /// The stream error, in words.
    pub async fn open(conn: quinn::Connection) -> Result<Self, String> {
        let (send, recv) = tokio::time::timeout(Duration::from_secs(5), conn.open_bi())
            .await
            .map_err(|_| "opening the control stream timed out".to_owned())?
            .map_err(|e| e.to_string())?;
        Ok(Self {
            conn,
            send,
            recv,
            decoder: FrameDecoder::new(Channel::Control, Side::Server),
            pending: Vec::new(),
            minor: 0,
        })
    }

    /// The negotiated minor version (0 before the `HelloAck`).
    #[must_use]
    pub fn minor(&self) -> u16 {
        self.minor
    }

    /// Sends one message.
    ///
    /// # Errors
    ///
    /// Encoding or stream error, in words.
    pub async fn send(&mut self, m: &Message) -> Result<(), String> {
        let bytes = frame_to_vec_minor(m, self.minor).map_err(|e| e.to_string())?;
        self.send_raw(&bytes).await
    }

    /// Sends raw bytes on the control stream, within 5 s.
    ///
    /// # Errors
    ///
    /// Stream error or timeout, in words.
    pub async fn send_raw(&mut self, bytes: &[u8]) -> Result<(), String> {
        match tokio::time::timeout(Duration::from_secs(5), self.send.write_all(bytes)).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e.to_string()),
            Err(_) => Err("write timed out".into()),
        }
    }

    /// Ends the sending side of the control stream (FIN).
    pub fn finish(&mut self) {
        let _ = self.send.finish();
    }

    /// The next control message within `timeout`: `Ok(None)` on timeout or end of stream.
    ///
    /// # Errors
    ///
    /// A decoding error (the server broke the protocol) or a stream error, in words.
    pub async fn recv(&mut self, timeout: Duration) -> Result<Option<Message>, String> {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut buf = vec![0u8; 8 * 1024];
        loop {
            if !self.pending.is_empty() {
                let mut input: &[u8] = &self.pending;
                let before = input.len();
                let r = self.decoder.push(&mut input);
                let used = before - input.len();
                self.pending.drain(..used);
                match r {
                    Ok(Some(m)) => {
                        if let Message::HelloAck(a) = &m {
                            // Frames after the HelloAck use the negotiated minor (§3.1).
                            self.minor = a.minor;
                            self.decoder.set_minor(a.minor);
                        }
                        return Ok(Some(m));
                    }
                    Ok(None) => {}
                    Err(e) => return Err(e.to_string()),
                }
            }
            match tokio::time::timeout_at(deadline, self.recv.read(&mut buf)).await {
                Err(_) => return Ok(None),
                Ok(Ok(Some(n))) => {
                    if self.pending.len() + n > MAX_PENDING {
                        return Err("control backlog too large".into());
                    }
                    self.pending.extend_from_slice(&buf[..n]);
                }
                Ok(Ok(None)) => return Ok(None),
                Ok(Err(e)) => return Err(e.to_string()),
            }
        }
    }

    /// Waits for a message `pick` accepts, skipping others, within `timeout`.
    ///
    /// # Errors
    ///
    /// As [`recv`](Self::recv); a `Disconnect` from the server is an error naming its
    /// code.
    pub async fn expect<T>(
        &mut self,
        timeout: Duration,
        mut pick: impl FnMut(&Message) -> Option<T>,
    ) -> Result<T, String> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(tokio::time::Instant::now());
            match self.recv(left).await? {
                Some(m) => {
                    if let Some(t) = pick(&m) {
                        return Ok(t);
                    }
                    if let Message::Disconnect(d) = &m {
                        return Err(format!("server disconnected: {}", d.code.name()));
                    }
                }
                None => return Err("no answer in time".into()),
            }
        }
    }

    /// Sends `Disconnect(NO_ERROR)` and closes the connection.
    pub async fn close(mut self) {
        let _ = self
            .send(&Message::Disconnect(Disconnect {
                code: AppErrorCode::NoError,
                text: String::new(),
            }))
            .await;
        self.finish();
        self.conn.close(0u32.into(), b"");
    }
}

/// The `Hello` the load tool sends (the highest minor it implements).
#[must_use]
pub fn hello() -> Message {
    hello_minor(tessera_protocol::limits::PROTOCOL_MINOR)
}

/// A `Hello` offering at most minor `minor_max`.
#[must_use]
pub fn hello_minor(minor_max: u16) -> Message {
    Message::Hello(Hello {
        minor_max,
        client_caps: 0,
        max_entities: 1024,
        client_name: format!("tessera-loadtest {}", env!("CARGO_PKG_VERSION")),
    })
}

/// The minor a bot offers: the highest this tool implements, except every tenth bot, which
/// stays at 0 (`test-server.md` §13.1: 10 % by default), and every tenth other bot, which
/// offers 1, so that observers at minors 0, 1 and 2 meet in every run.
#[must_use]
pub fn bot_minor(index: usize) -> u16 {
    match index % 10 {
        9 => 0,
        8 => 1,
        _ => tessera_protocol::limits::PROTOCOL_MINOR,
    }
}

/// An invented look for a bot: made-up property IDs and values chosen by the tool, never
/// resolved (`test-server.md` §13.1).
#[must_use]
pub fn bot_look(index: usize) -> Look {
    let i = u32::try_from(index % 1_000).unwrap_or(0);
    Look {
        entries: vec![
            LookEntry {
                property: 0x7FF0_0001,
                value: LookValue::Float32(f32::from(u16::try_from(i % 100).unwrap_or(0)) / 100.0),
            },
            LookEntry {
                property: 0x7FF0_0002,
                value: LookValue::Colour([(i % 256) as u8, 128, 64, 255]),
            },
            LookEntry {
                property: 0x7FF0_0003,
                value: LookValue::String(format!("bot{i}")),
            },
        ],
    }
}

/// An invented appearance for a bot: numbers chosen by the tool, never resolved
/// (`test-server.md` §13.1).
#[must_use]
pub fn bot_appearance(index: usize) -> Appearance {
    let mut a = Appearance::new(Body::Character {
        species: 1 + (index % 7) as u32,
        sex: 1 + (index % 2) as u32,
    });
    // An invented held item with its slot key (sent only at minor ≥ 1).
    a.equipment.push(Equipped {
        item: 0x7000_0000 + (index % 97) as u32 + 1,
        slot: Some(17 + (index % 2) as u8),
        ..Equipped::default()
    });
    a
}

/// What [`provision`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provisioned {
    /// The character already existed.
    Existing,
    /// The character was created.
    Created,
}

/// Logs in (or registers) and creates `character` with the given appearance and look
/// unless the account already has it.
///
/// # Errors
///
/// What failed, in words (connection, login code, creation code).
pub async fn provision(
    endpoint: &quinn::Endpoint,
    target: &Target,
    account: &str,
    password: &str,
    character: &str,
    register: bool,
    (appearance, look): (Appearance, Look),
) -> Result<Provisioned, String> {
    let wait = Duration::from_secs(30);
    let mut attempt = 0u32;
    let characters = loop {
        attempt += 1;
        let conn = connect(endpoint, target, Duration::from_secs(10)).await?;
        let mut link = ControlLink::open(conn).await?;
        link.send(&hello()).await?;
        link.expect(wait, |m| matches!(m, Message::HelloAck(_)).then_some(()))
            .await?;
        let creds = Credentials {
            account: account.to_owned(),
            password: password.to_owned(),
        };
        let mut use_register = register;
        let answer = loop {
            let msg = if use_register {
                Message::Register(creds.clone())
            } else {
                Message::Login(creds.clone())
            };
            link.send(&msg).await?;
            let a = link
                .expect(wait, |m| match m {
                    Message::LoginOk(ok) => Some(Ok(ok.characters.clone())),
                    Message::LoginFailed(f) => Some(Err((f.code, f.retry_after))),
                    _ => None,
                })
                .await?;
            match a {
                Err((LoginFailedCode::AccountExists, _)) if use_register => use_register = false,
                other => break other,
            }
        };
        match answer {
            Ok(chars) => break (link, chars),
            Err((LoginFailedCode::RateLimited, after)) if attempt < 6 => {
                link.close().await;
                tokio::time::sleep(Duration::from_secs(u64::from(after.clamp(1, 30)))).await;
            }
            Err((code, _)) => {
                link.close().await;
                return Err(format!("login failed: {}", code.name()));
            }
        }
    };
    let (mut link, characters) = characters;
    if characters.iter().any(|c| c.name == character) {
        link.close().await;
        return Ok(Provisioned::Existing);
    }
    link.send(&Message::CharacterCreate(CharacterCreate {
        name: character.to_owned(),
        appearance,
        look,
    }))
    .await?;
    let r = link
        .expect(wait, |m| match m {
            Message::CharacterList(_) => Some(Ok(())),
            Message::CharacterCreateFailed(f) => Some(Err(f.code.name())),
            _ => None,
        })
        .await;
    link.close().await;
    match r? {
        Ok(()) => Ok(Provisioned::Created),
        Err(code) => Err(format!("character creation failed: {code}")),
    }
}

/// One client-to-server negative vector of `tessera-protocol.md` §16.8: the minor the
/// `Hello` offers, the frame sent right after it, and a name for reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NegativeVector {
    /// Short name.
    pub name: &'static str,
    /// `Hello.minor_max`.
    pub minor_max: u16,
    /// The frame (length, type, body).
    pub frame: Vec<u8>,
}

fn with_length(mut frame: Vec<u8>) -> Vec<u8> {
    let len = u32::try_from(frame.len().saturating_sub(4)).unwrap_or(u32::MAX);
    if let Some(head) = frame.get_mut(..4) {
        head.copy_from_slice(&len.to_le_bytes());
    }
    frame
}

/// `CharacterCreate` "Aldith", Character 23/1, scale 1, no idle, no items, then `look`.
fn create_with_look(look: &[u8]) -> Vec<u8> {
    let mut f = vec![
        0, 0, 0, 0, 0x07, 0x00, 0x06, 0x00, b'A', b'l', b'd', b'i', b't', b'h', 0x01, 0x17, 0, 0,
        0, 0x01, 0, 0, 0, 0, 0, 0x80, 0x3f, 0x00, 0x00,
    ];
    f.extend_from_slice(look);
    with_length(f)
}

/// `AppearanceSet` Character 23/1 with one item 0x70001234 whose flags and slot follow.
fn appearance_set_item(flags: u8, slot: Option<u8>) -> Vec<u8> {
    let mut f = vec![
        0, 0, 0, 0, 0x11, 0x00, 0x01, 0x17, 0, 0, 0, 0x01, 0, 0, 0, 0, 0, 0x80, 0x3f, 0x00, 0x01,
        0x34, 0x12, 0x00, 0x70, flags,
    ];
    f.extend(slot);
    with_length(f)
}

/// Every client-to-server negative vector of §16.8 (the `looks` count case travels server
/// to client and is a test of the client's decoder). Each MUST end the connection with
/// `PROTOCOL_VIOLATION` (`test-server.md` §13.1 "Hostile 1.1 cases").
#[must_use]
pub fn negative_vectors_v11() -> Vec<NegativeVector> {
    let k: [u8; 32] = [
        0x03, 0x01, 0x0a, 0x00, 0x10, 0x04, 0x00, 0x00, 0x00, 0x3f, 0x02, 0x0a, 0x00, 0x10, 0x0d,
        0xc8, 0x96, 0x64, 0xff, 0x03, 0x0a, 0x00, 0x10, 0x07, 0x06, 0x00, 0x38, 0x30, 0x38, 0x30,
        0x66, 0x66,
    ];
    let mut many = vec![49u8];
    for i in 1..=49u32 {
        many.extend(i.to_le_bytes());
        many.extend([0x01, 0x01]);
    }
    let mut big = vec![6u8];
    for i in 1..=6u32 {
        big.extend(i.to_le_bytes());
        big.extend([0x07, 0x80, 0x00]);
        big.extend(std::iter::repeat_n(b'A', 128));
    }
    let v = |name, minor_max, frame| NegativeVector {
        name,
        minor_max,
        frame,
    };
    vec![
        v("slot-at-minor-0", 0, appearance_set_item(0x08, Some(17))),
        v("look-at-minor-0", 0, create_with_look(&k)),
        v("slot-0", 1, appearance_set_item(0x08, Some(0x00))),
        v("slot-65", 1, appearance_set_item(0x08, Some(0x41))),
        v("flag-bit-4", 1, appearance_set_item(0x10, None)),
        v(
            "look-unordered",
            1,
            create_with_look(&[
                0x02, 0x02, 0x0a, 0x00, 0x10, 0x01, 0x01, 0x01, 0x0a, 0x00, 0x10, 0x01, 0x00,
            ]),
        ),
        v(
            "look-pid-0",
            1,
            create_with_look(&[0x01, 0x00, 0x00, 0x00, 0x00, 0x01, 0x01]),
        ),
        v(
            "look-array-tag",
            1,
            create_with_look(&[0x01, 0x01, 0x0a, 0x00, 0x10, 0x0e, 0x00, 0x00]),
        ),
        v(
            "look-space",
            1,
            create_with_look(&[
                0x01, 0x01, 0x0a, 0x00, 0x10, 0x07, 0x03, 0x00, 0x61, 0x20, 0x62,
            ]),
        ),
        v("look-49-entries", 1, create_with_look(&many)),
        v("look-811-bytes", 1, create_with_look(&big)),
    ]
}

/// Whether a client-to-server vector needs a server that advertises `CHAT_CHANNELS`
/// (protocol minor 2, `tessera-protocol.md` §16.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelsNeeded {
    /// The server advertises named channels (the default).
    On,
    /// The server runs with `[chat] channels = false`.
    Off,
}

/// `ChannelJoin` "Lore" (`tessera-protocol.md` §16.9).
const CHANNEL_JOIN_LORE: [u8; 12] = [
    0x08, 0, 0, 0, 0x15, 0x00, 0x04, 0x00, b'L', b'o', b'r', b'e',
];
/// `ChatSend` Named "Lore", "hi" (§16.9).
const CHAT_SEND_LORE: [u8; 17] = [
    0x0d, 0, 0, 0, 0x0e, 0x00, 0x06, 0x04, 0x00, b'L', b'o', b'r', b'e', 0x02, 0x00, b'h', b'i',
];

/// `ChannelJoin` or `ChannelLeave` (`ty`) carrying exactly `name` (any bytes, even ones that
/// are not UTF-8 or longer than a `NAME`).
#[must_use]
pub fn channel_request(ty: u16, name: &[u8]) -> Vec<u8> {
    let mut f = vec![0, 0, 0, 0];
    f.extend(ty.to_le_bytes());
    f.extend(u16::try_from(name.len()).unwrap_or(u16::MAX).to_le_bytes());
    f.extend_from_slice(name);
    with_length(f)
}

/// The client-to-server negative vectors of `tessera-protocol.md` §16.9 (protocol 1.2:
/// named channels), with the server setting each needs. Each MUST end the connection with
/// `PROTOCOL_VIOLATION` when sent in the world (`test-server.md` §13.1 "Hostile 1.2
/// cases").
#[must_use]
pub fn negative_vectors_v12() -> Vec<(NegativeVector, ChannelsNeeded)> {
    let v = |name, minor_max, frame| NegativeVector {
        name,
        minor_max,
        frame,
    };
    let mut seven = CHAT_SEND_LORE.to_vec();
    seven[6] = 7;
    let mut trailing = CHANNEL_JOIN_LORE.to_vec();
    trailing.push(0);
    vec![
        (
            v("join-at-minor-1", 1, CHANNEL_JOIN_LORE.to_vec()),
            ChannelsNeeded::On,
        ),
        (
            v("named-line-at-minor-1", 1, CHAT_SEND_LORE.to_vec()),
            ChannelsNeeded::On,
        ),
        (
            v("join-without-capability", 2, CHANNEL_JOIN_LORE.to_vec()),
            ChannelsNeeded::Off,
        ),
        (
            v("named-line-without-capability", 2, CHAT_SEND_LORE.to_vec()),
            ChannelsNeeded::Off,
        ),
        (v("chat-channel-7", 2, seven), ChannelsNeeded::On),
        (
            v(
                "channel-name-65-bytes",
                2,
                channel_request(0x0015, &[b'A'; 65]),
            ),
            ChannelsNeeded::On,
        ),
        (
            v("join-trailing-byte", 2, with_length(trailing)),
            ChannelsNeeded::On,
        ),
    ]
}

/// A client in the world on one connection, reading its chat stream (protocol minor ≥ 0).
/// Its world stream is read and discarded by a task of the current runtime, so the server's
/// world backlog never grows; the task ends with the connection.
#[derive(Debug)]
pub struct WorldClient {
    /// The control stream.
    pub link: ControlLink,
    chat: quinn::RecvStream,
    decoder: FrameDecoder,
    pending: Vec<u8>,
    world: tokio::task::JoinHandle<()>,
}

async fn accept_kind(
    conn: &quinn::Connection,
    want: StreamKind,
    timeout: Duration,
) -> Result<quinn::RecvStream, String> {
    let mut s = match tokio::time::timeout(timeout, conn.accept_uni()).await {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => return Err(e.to_string()),
        Err(_) => return Err("the server opened no stream in time".into()),
    };
    let mut kind = [0u8; 1];
    tokio::time::timeout(timeout, s.read_exact(&mut kind))
        .await
        .map_err(|_| "no kind byte in time".to_owned())?
        .map_err(|e| e.to_string())?;
    match StreamKind::from_byte(kind[0]) {
        Ok(k) if k == want => Ok(s),
        other => Err(format!("expected the {want:?} stream, got {other:?}")),
    }
}

impl WorldClient {
    /// Connects, offers minor `minor_max`, logs in and enters the world with `character`.
    ///
    /// # Errors
    ///
    /// What failed, in words.
    pub async fn enter(
        endpoint: &quinn::Endpoint,
        target: &Target,
        (account, password): (&str, &str),
        character: &str,
        minor_max: u16,
        timeout: Duration,
    ) -> Result<Self, String> {
        let conn = connect(endpoint, target, timeout).await?;
        let mut link = ControlLink::open(conn).await?;
        link.send(&hello_minor(minor_max)).await?;
        link.expect(timeout, |m| matches!(m, Message::HelloAck(_)).then_some(()))
            .await?;
        link.send(&Message::Login(Credentials {
            account: account.to_owned(),
            password: password.to_owned(),
        }))
        .await?;
        link.expect(timeout, |m| match m {
            Message::LoginOk(_) => Some(Ok(())),
            Message::LoginFailed(f) => Some(Err(f.code.name())),
            _ => None,
        })
        .await?
        .map_err(|code| format!("login failed: {code}"))?;
        // The chat stream follows LoginOk; the world stream follows EnterWorld.
        let chat = accept_kind(&link.conn, StreamKind::Chat, timeout).await?;
        link.send(&Message::EnterWorld(EnterWorld {
            name: character.to_owned(),
        }))
        .await?;
        let mut world = accept_kind(&link.conn, StreamKind::World, timeout).await?;
        let world = tokio::spawn(async move {
            let mut buf = vec![0u8; 16 * 1024];
            while let Ok(Some(_)) = world.read(&mut buf).await {}
        });
        let minor = link.minor();
        Ok(Self {
            link,
            chat,
            decoder: FrameDecoder::with_minor(Channel::Chat, Side::Server, minor),
            pending: Vec::new(),
            world,
        })
    }

    /// The negotiated minor version.
    #[must_use]
    pub fn minor(&self) -> u16 {
        self.link.minor()
    }

    /// The next chat-stream message within `timeout`: `Ok(None)` on timeout or end of
    /// stream.
    ///
    /// # Errors
    ///
    /// A decoding error (the server broke the protocol at the negotiated minor) or a stream
    /// error, in words.
    pub async fn next_chat(&mut self, timeout: Duration) -> Result<Option<Message>, String> {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut buf = vec![0u8; 8 * 1024];
        loop {
            if !self.pending.is_empty() {
                let mut input: &[u8] = &self.pending;
                let before = input.len();
                let r = self.decoder.push(&mut input);
                let used = before - input.len();
                self.pending.drain(..used);
                match r {
                    Ok(Some(m)) => return Ok(Some(m)),
                    Ok(None) => {}
                    Err(e) => return Err(e.to_string()),
                }
            }
            match tokio::time::timeout_at(deadline, self.chat.read(&mut buf)).await {
                Err(_) | Ok(Ok(None)) => return Ok(None),
                Ok(Ok(Some(n))) => {
                    if self.pending.len() + n > MAX_PENDING {
                        return Err("chat backlog too large".into());
                    }
                    self.pending.extend_from_slice(&buf[..n]);
                }
                Ok(Err(e)) => return Err(e.to_string()),
            }
        }
    }

    /// Waits for a chat-stream message `pick` accepts within `timeout`; every message seen
    /// before it is appended to `skipped`.
    ///
    /// # Errors
    ///
    /// As [`next_chat`](Self::next_chat), or none in time.
    pub async fn expect_chat<T>(
        &mut self,
        timeout: Duration,
        skipped: &mut Vec<Message>,
        mut pick: impl FnMut(&Message) -> Option<T>,
    ) -> Result<T, String> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(tokio::time::Instant::now());
            match self.next_chat(left).await? {
                Some(m) => match pick(&m) {
                    Some(t) => return Ok(t),
                    None => skipped.push(m),
                },
                None => return Err("no chat message in time".into()),
            }
        }
    }

    /// Stops the world reader, sends `Disconnect(NO_ERROR)` and closes the connection.
    pub async fn close(self) {
        self.world.abort();
        self.link.close().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_protocol::frame::FrameDecoder;

    /// Each vector is refused by the protocol's own decoder at the minor it negotiates,
    /// and its v1.1-only content decodes at the other minor where §16.8 says so.
    #[test]
    fn vectors_are_refused_by_the_decoder() {
        use tessera_protocol::ProtocolError as E;
        let want = [
            E::ReservedBits("equipped.flags"),
            E::TrailingBytes(32),
            E::OutOfRange("equipped.slot"),
            E::OutOfRange("equipped.slot"),
            E::ReservedBits("equipped.flags"),
            E::IdOrder("look"),
            E::IdOrder("look"),
            E::LookTag(0x0E),
            E::LookString,
            E::Count {
                field: "look",
                count: 49,
                min: 0,
                max: 48,
            },
            E::LookTooLarge(811),
        ];
        for (v, want) in negative_vectors_v11().into_iter().zip(want) {
            let mut d = FrameDecoder::with_minor(Channel::Control, Side::Client, v.minor_max);
            let mut input: &[u8] = &v.frame;
            assert_eq!(d.push(&mut input), Err(want), "{}", v.name);
        }
        // The two minor-0 vectors are valid at minor 1.
        for v in negative_vectors_v11().iter().take(2) {
            let mut d = FrameDecoder::with_minor(Channel::Control, Side::Client, 1);
            let mut input: &[u8] = &v.frame;
            assert!(d.push(&mut input).unwrap().is_some(), "{}", v.name);
        }
    }

    /// The §16.9 vectors: the decoding ones are refused by the protocol's own decoder at
    /// their minor; the capability ones decode and are refused by the session (no
    /// `CHAT_CHANNELS`), which the end-to-end test checks.
    #[test]
    fn v12_vectors_are_refused() {
        use tessera_protocol::ProtocolError as E;
        let vectors = negative_vectors_v12();
        assert_eq!(vectors.len(), 7);
        for (v, needs) in &vectors {
            let mut d = FrameDecoder::with_minor(Channel::Control, Side::Client, v.minor_max);
            let mut input: &[u8] = &v.frame;
            let r = d.push(&mut input);
            match v.name {
                "join-at-minor-1" => assert_eq!(r, Err(E::UnknownMessageType(0x0015))),
                "named-line-at-minor-1" => assert_eq!(
                    r,
                    Err(E::InvalidEnum {
                        field: "channel",
                        value: 6
                    })
                ),
                "chat-channel-7" => assert_eq!(
                    r,
                    Err(E::InvalidEnum {
                        field: "channel",
                        value: 7
                    })
                ),
                "channel-name-65-bytes" => {
                    assert!(matches!(r, Err(E::StringLength { len: 65, .. })), "{r:?}");
                }
                "join-trailing-byte" => assert_eq!(r, Err(E::TrailingBytes(1))),
                _ => {
                    assert_eq!(*needs, ChannelsNeeded::Off, "{}", v.name);
                    assert!(matches!(r, Ok(Some(_))), "{}: {r:?}", v.name);
                }
            }
        }
        assert_eq!(
            channel_request(0x0016, b"Lore"),
            [0x08, 0, 0, 0, 0x16, 0, 4, 0, b'L', b'o', b'r', b'e']
        );
    }

    #[test]
    fn bot_looks_and_appearances_encode() {
        for i in [0, 9, 10, 999, 1_000_000] {
            assert!(bot_look(i).check().is_ok());
            let m = Message::CharacterCreate(CharacterCreate {
                name: "B".into(),
                appearance: bot_appearance(i),
                look: bot_look(i),
            });
            assert!(frame_to_vec_minor(&m, 1).is_ok());
            assert!(frame_to_vec_minor(&m, 0).is_ok());
        }
        assert_eq!(bot_minor(9), 0);
        assert_eq!(bot_minor(8), 1);
        assert_eq!(bot_minor(0), tessera_protocol::limits::PROTOCOL_MINOR);
        // Every minor up to the tool's own meets among ten consecutive bots.
        let minors: std::collections::BTreeSet<u16> = (0..10).map(bot_minor).collect();
        assert_eq!(
            minors,
            (0..=tessera_protocol::limits::PROTOCOL_MINOR).collect()
        );
    }
}
