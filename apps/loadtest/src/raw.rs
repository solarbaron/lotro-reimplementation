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
use tessera_protocol::frame::{FrameDecoder, frame_to_vec};
use tessera_protocol::message::{CharacterCreate, Credentials, Disconnect, Hello};
use tessera_protocol::transport::{PinnedServerVerifier, client_endpoint, quinn_client_config};
use tessera_protocol::types::{Appearance, Body};
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
        })
    }

    /// Sends one message.
    ///
    /// # Errors
    ///
    /// Encoding or stream error, in words.
    pub async fn send(&mut self, m: &Message) -> Result<(), String> {
        let bytes = frame_to_vec(m).map_err(|e| e.to_string())?;
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
                    Ok(Some(m)) => return Ok(Some(m)),
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

/// The `Hello` the load tool sends.
#[must_use]
pub fn hello() -> Message {
    Message::Hello(Hello {
        minor_max: tessera_protocol::limits::PROTOCOL_MINOR,
        client_caps: 0,
        max_entities: 1024,
        client_name: format!("tessera-loadtest {}", env!("CARGO_PKG_VERSION")),
    })
}

/// An invented appearance for a bot: numbers chosen by the tool, never resolved
/// (`test-server.md` §13.1).
#[must_use]
pub fn bot_appearance(index: usize) -> Appearance {
    Appearance::new(Body::Character {
        species: 1 + (index % 7) as u32,
        sex: 1 + (index % 2) as u32,
    })
}

/// What [`provision`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provisioned {
    /// The character already existed.
    Existing,
    /// The character was created.
    Created,
}

/// Logs in (or registers) and creates `character` unless the account already has it.
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
    appearance: Appearance,
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
