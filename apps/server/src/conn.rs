// SPDX-License-Identifier: GPL-3.0-or-later
//! Per-connection inbound handling without sockets: decoding and the protocol state
//! machine ([`ServerSession`], `tessera-protocol.md` §12.2) plus the server's own admission
//! rule (`SERVER_FULL` in answer to `Hello`, `test-server.md` §10.1).
//!
//! The network task feeds a [`ConnectionCore`] with received bytes and the simulation's
//! [`SessionCmd`]s and sends what [`ConnectionCore::poll_transmit`] returns. The fuzz targets
//! drive it the same way without sockets.
//!
//! Two orderings are kept here:
//!
//! - **Control back-pressure is per message** (§10.1 `control_queue`): received control
//!   bytes are buffered ([`ConnectionCore::push_control`]) and fed to the session one frame
//!   at a time ([`ConnectionCore::decode_control`]), so the network task can stop after any
//!   number of forwarded events, not only after a whole read.
//! - **`ChatRejected` answers `ChatSend`s in order** (`tessera-protocol.md` §6.4.3, §10.6):
//!   a rejection the session makes itself (rate limit, invalid text or peer name) is held
//!   until every earlier `ChatSend`, `ChannelJoin` and `ChannelLeave` forwarded to the
//!   simulation has been answered, by [`SessionCmd::RejectChat`],
//!   [`SessionCmd::ChatDelivered`], [`SessionCmd::ChannelJoined`],
//!   [`SessionCmd::ChannelLeft`] or [`SessionCmd::RefuseChannel`]. The simulation answers
//!   the forwarded requests in the order it receives them.

use std::collections::VecDeque;

use tessera_protocol::Message;
use tessera_protocol::codes::AppErrorCode;
use tessera_protocol::frame::parse_frame_length;
use tessera_protocol::limits::FRAME_LENGTH_FIELD;
use tessera_protocol::message::{ChannelLeftReason, Disconnect, Hello, MoveInput};
use tessera_protocol::server::{ServerEvent, ServerParams, ServerSession, ServerState};
use tessera_protocol::session::{ConnectionEnd, PeerDatagrams, StreamEnd, StreamRef, Transmit};

use crate::shared::SessionCmd;

/// Something the network task forwards.
#[derive(Debug, Clone, PartialEq)]
pub enum Inbound {
    /// The client's `Hello` was answered with `HelloAck` (never forwarded for a refused
    /// connection).
    Greeted(Hello),
    /// A control event for the simulation.
    Event(ServerEvent),
    /// A `MoveInput` for the newest-wins movement slot.
    Move(MoveInput),
    /// The session ended (exactly once): with this code, and whether the client initiated
    /// it.
    Ended {
        /// The code.
        code: AppErrorCode,
        /// Whether the client ended it.
        client: bool,
        /// Whether the client broke a rule of the protocol.
        violation: bool,
    },
}

/// Session-made chat rejections held behind unanswered `ChatSend`s, at most. Beyond it a
/// rejection is sent at once (the session's flood rule ends such a connection long before).
const MAX_HELD_REJECTIONS: usize = 256;

/// Finds frame boundaries in the control byte stream (`tessera-protocol.md` §5.4: a 4-byte
/// length, then that many bytes). Only the boundaries: decoding stays with the session.
#[derive(Debug, Default)]
struct FrameSplitter {
    header: [u8; FRAME_LENGTH_FIELD],
    have: usize,
    body_left: usize,
    broken: bool,
}

impl FrameSplitter {
    /// The number of leading bytes of `data` up to and including the end of the next frame,
    /// or all of `data` when no frame ends in it (or the stream is already malformed, which
    /// the session then reports).
    fn next_len(&mut self, data: &[u8]) -> usize {
        let mut i = 0;
        while i < data.len() {
            if self.broken {
                return data.len();
            }
            if self.have < FRAME_LENGTH_FIELD {
                self.header[self.have] = data[i];
                self.have += 1;
                i += 1;
                if self.have == FRAME_LENGTH_FIELD {
                    match parse_frame_length(self.header) {
                        Ok(len) => self.body_left = len,
                        Err(_) => {
                            self.broken = true;
                            return data.len();
                        }
                    }
                }
                continue;
            }
            let take = self.body_left.min(data.len() - i);
            self.body_left -= take;
            i += take;
            if self.body_left == 0 {
                self.have = 0;
                return i;
            }
        }
        data.len()
    }
}

/// An entry of the chat answer order.
#[derive(Debug)]
enum ChatSlot {
    /// A `ChatSend`, `ChannelJoin` or `ChannelLeave` forwarded to the simulation and not
    /// answered yet.
    Forwarded,
    /// A session-made `ChatRejected` waiting for the forwarded ones before it.
    Held(Box<Transmit>),
}

/// One connection's protocol state.
#[derive(Debug)]
pub struct ConnectionCore {
    session: ServerSession,
    refused: bool,
    ended: bool,
    /// Control bytes received and not yet fed to the session.
    unread: Vec<u8>,
    unread_pos: usize,
    splitter: FrameSplitter,
    /// What the session queued, in order, minus held chat rejections.
    out: VecDeque<Transmit>,
    chat_order: VecDeque<ChatSlot>,
    held: usize,
}

impl ConnectionCore {
    /// A connection whose QUIC handshake started at `now`.
    #[must_use]
    pub fn new(params: ServerParams, now: u64) -> Self {
        Self {
            session: ServerSession::new(params, now),
            refused: false,
            ended: false,
            unread: Vec::new(),
            unread_pos: 0,
            splitter: FrameSplitter::default(),
            out: VecDeque::new(),
            chat_order: VecDeque::new(),
            held: 0,
        }
    }

    /// Marks the connection as refused by admission: its `Hello` is answered with
    /// `Disconnect(SERVER_FULL)` instead of `HelloAck` (§10.1).
    pub fn refuse(&mut self) {
        self.refused = true;
    }

    /// The protocol state.
    #[must_use]
    pub fn state(&self) -> ServerState {
        self.session.state()
    }

    /// The negotiated minor version (0 before `Hello`); frames of
    /// [`poll_transmit`](Self::poll_transmit) are encoded under it (`tessera-protocol.md`
    /// §3.1).
    #[must_use]
    pub fn minor(&self) -> u16 {
        self.session.minor()
    }

    /// Whether the session ended.
    #[must_use]
    pub fn ended(&self) -> bool {
        self.ended
    }

    /// Whether world-stream messages, chat-stream messages and `MotionBatch`es from the
    /// simulation may still be sent.
    #[must_use]
    pub fn may_send_world(&self) -> bool {
        self.session.state() == ServerState::InWorld
    }

    /// Whether chat-stream messages may still be sent.
    #[must_use]
    pub fn may_send_chat(&self) -> bool {
        matches!(
            self.session.state(),
            ServerState::CharacterSelect | ServerState::InWorld
        )
    }

    /// The client's `Disconnect` the session acted on, if any (`test-server.md` §11.1, the
    /// cause of the end).
    #[must_use]
    pub fn peer_disconnect(&self) -> Option<&Disconnect> {
        self.session.peer_disconnect()
    }

    /// Messages dropped under §13.2.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.session.dropped()
    }

    /// The next thing to transmit.
    pub fn poll_transmit(&mut self) -> Option<Transmit> {
        self.drain(false);
        self.out.pop_front()
    }

    /// Moves what the session queued to the output. With `local`, a `ChatRejected` is one
    /// the session made itself while decoding: it waits behind unanswered forwarded
    /// `ChatSend`s.
    fn drain(&mut self, local: bool) {
        while let Some(t) = self.session.poll_transmit() {
            let rejection = matches!(&t, Transmit::Message(Message::ChatRejected(_)));
            if local
                && rejection
                && self.held < MAX_HELD_REJECTIONS
                && self
                    .chat_order
                    .iter()
                    .any(|s| matches!(s, ChatSlot::Forwarded))
            {
                self.held += 1;
                self.chat_order.push_back(ChatSlot::Held(Box::new(t)));
            } else {
                self.out.push_back(t);
            }
        }
    }

    /// The oldest forwarded `ChatSend` was answered: release the rejections held behind it.
    fn chat_answered(&mut self) {
        if let Some(i) = self
            .chat_order
            .iter()
            .position(|s| matches!(s, ChatSlot::Forwarded))
        {
            self.chat_order.remove(i);
        }
        while matches!(self.chat_order.front(), Some(ChatSlot::Held(_))) {
            if let Some(ChatSlot::Held(t)) = self.chat_order.pop_front() {
                self.held = self.held.saturating_sub(1);
                self.out.push_back(*t);
            }
        }
    }

    /// `ChatSend`s forwarded to the simulation and not answered yet.
    #[must_use]
    pub fn unanswered_chats(&self) -> usize {
        self.chat_order
            .iter()
            .filter(|s| matches!(s, ChatSlot::Forwarded))
            .count()
    }

    /// When [`handle_timeout`](Self::handle_timeout) is due, µs.
    #[must_use]
    pub fn poll_timeout(&self) -> Option<u64> {
        self.session.poll_timeout()
    }

    fn ended_event(
        &mut self,
        code: AppErrorCode,
        client: bool,
        violation: bool,
    ) -> Option<Inbound> {
        if self.ended {
            return None;
        }
        self.ended = true;
        Some(Inbound::Ended {
            code,
            client,
            violation,
        })
    }

    fn map_event(&mut self, e: ServerEvent, now: u64, out: &mut Vec<Inbound>) {
        match e {
            ServerEvent::Closed { code, client } => {
                if let Some(ev) = self.ended_event(code, client, false) {
                    out.push(ev);
                }
            }
            ServerEvent::Pong { .. } => {}
            ServerEvent::Move(m) => out.push(Inbound::Move(m)),
            ServerEvent::Greeted(h) => {
                if self.refused {
                    // Drop the queued HelloAck: the answer to Hello is Disconnect(SERVER_FULL).
                    while self.session.poll_transmit().is_some() {}
                    if let Some(ServerEvent::Closed { code, client }) =
                        self.session.disconnect(AppErrorCode::ServerFull, "", now)
                        && let Some(ev) = self.ended_event(code, client, false)
                    {
                        out.push(ev);
                    }
                } else {
                    out.push(Inbound::Greeted(h));
                }
            }
            other => {
                if !self.ended {
                    if matches!(
                        other,
                        ServerEvent::Chat(_)
                            | ServerEvent::ChannelJoin { .. }
                            | ServerEvent::ChannelLeave { .. }
                    ) {
                        self.chat_order.push_back(ChatSlot::Forwarded);
                    }
                    out.push(Inbound::Event(other));
                }
            }
        }
    }

    fn violation(&mut self, v: &tessera_protocol::session::Violation, out: &mut Vec<Inbound>) {
        tracing::debug!(error = %v, "client broke the protocol");
        let code = v.app_code();
        if let Some(ev) = self.ended_event(code, false, code == AppErrorCode::ProtocolViolation) {
            out.push(ev);
        }
    }

    /// The QUIC handshake completed.
    pub fn handshake_complete(&mut self, now: u64, datagrams: PeerDatagrams) -> Vec<Inbound> {
        let mut out = Vec::new();
        match self.session.handshake_complete(now, datagrams) {
            Ok(Some(e)) => self.map_event(e, now, &mut out),
            Ok(None) => {}
            Err(e) => tracing::debug!(error = %e, "handshake_complete in the wrong state"),
        }
        self.drain(false);
        out
    }

    /// Bytes read from the control stream, decoded at once (every complete frame).
    pub fn on_control_data(&mut self, data: &[u8], now: u64) -> Vec<Inbound> {
        self.push_control(data);
        self.decode_control(now, usize::MAX)
    }

    /// Buffers bytes read from the control stream without decoding them. The network task
    /// reads again only once [`has_unread_control`](Self::has_unread_control) is false, so
    /// the buffer holds at most one read.
    pub fn push_control(&mut self, data: &[u8]) {
        if self.unread_pos >= self.unread.len() {
            self.unread.clear();
            self.unread_pos = 0;
        }
        self.unread.extend_from_slice(data);
    }

    /// Whether buffered control bytes wait to be decoded.
    #[must_use]
    pub fn has_unread_control(&self) -> bool {
        self.unread_pos < self.unread.len()
    }

    /// Feeds buffered control bytes to the session one frame at a time until `max_events`
    /// events for the simulation ([`Inbound::Greeted`], [`Inbound::Event`]) were produced or
    /// the buffer is empty.
    pub fn decode_control(&mut self, now: u64, max_events: usize) -> Vec<Inbound> {
        let mut out = Vec::new();
        let mut events = 0usize;
        while self.unread_pos < self.unread.len() && events < max_events {
            let start = self.unread_pos;
            let n = self.splitter.next_len(&self.unread[start..]).max(1);
            let end = (start + n).min(self.unread.len());
            self.unread_pos = end;
            let before = out.len();
            match self.session.on_control_data(&self.unread[start..end], now) {
                Ok(evs) => {
                    for e in evs {
                        self.map_event(e, now, &mut out);
                    }
                }
                Err(v) => self.violation(&v, &mut out),
            }
            events += out[before..]
                .iter()
                .filter(|i| matches!(i, Inbound::Event(_) | Inbound::Greeted(_)))
                .count();
            self.drain(true);
        }
        if self.unread_pos >= self.unread.len() {
            self.unread.clear();
            self.unread_pos = 0;
        }
        out
    }

    /// One datagram.
    pub fn on_datagram(&mut self, data: &[u8], now: u64) -> Vec<Inbound> {
        let mut out = Vec::new();
        match self.session.on_datagram(data, now) {
            Ok(Some(e)) => self.map_event(e, now, &mut out),
            Ok(None) => {}
            Err(v) => self.violation(&v, &mut out),
        }
        self.drain(false);
        out
    }

    /// The client finished, reset or stopped a stream.
    pub fn on_stream_end(&mut self, stream: StreamRef, end: StreamEnd) -> Vec<Inbound> {
        let mut out = Vec::new();
        if let Err(v) = self.session.on_stream_end(stream, end) {
            self.violation(&v, &mut out);
        }
        out
    }

    /// The client opened a stream beyond its control stream.
    pub fn on_extra_stream(&mut self) -> Vec<Inbound> {
        let mut out = Vec::new();
        if let Err(v) = self.session.on_extra_stream() {
            self.violation(&v, &mut out);
        }
        out
    }

    /// A deadline passed.
    pub fn handle_timeout(&mut self, now: u64) -> Vec<Inbound> {
        let mut out = Vec::new();
        if let Some(e) = self.session.handle_timeout(now) {
            self.map_event(e, now, &mut out);
        }
        out
    }

    /// The QUIC connection closed.
    pub fn connection_closed(&mut self, end: ConnectionEnd) -> Vec<Inbound> {
        let mut out = Vec::new();
        if let Some(e) = self.session.connection_closed(end) {
            self.map_event(e, 0, &mut out);
        }
        // A session that ended without reporting (for example a violation already
        // reported) is over now in any case.
        if let Some(ev) = self.ended_event(AppErrorCode::NoError, false, false) {
            out.push(ev);
        }
        out
    }

    /// Ends the session locally (`Disconnect(code)` and the closing procedure).
    pub fn disconnect(&mut self, code: AppErrorCode, text: &str, now: u64) -> Vec<Inbound> {
        let mut out = Vec::new();
        if let Some(e) = self.session.disconnect(code, text, now) {
            self.map_event(e, now, &mut out);
        }
        out
    }

    /// Applies a request from the simulation. Requests the state no longer allows (the
    /// session ended meanwhile) are dropped.
    pub fn apply(&mut self, cmd: SessionCmd, now: u64) -> Vec<Inbound> {
        let mut out = Vec::new();
        let result = match cmd {
            SessionCmd::AcceptLogin {
                account,
                characters,
                looks,
            } => self
                .session
                .accept_login_with_looks(account, characters, looks, now)
                .map(|()| None),
            SessionCmd::RejectLogin { code, retry_after } => {
                self.session.reject_login(code, retry_after, "", now)
            }
            SessionCmd::CharacterCreated { characters, looks } => self
                .session
                .character_created_with_looks(characters, looks)
                .map(|()| None),
            SessionCmd::RejectCharacter(code) => {
                self.session.reject_character(code, "").map(|()| None)
            }
            SessionCmd::EnterWorld(enter) => self.session.enter_world(*enter).map(|()| None),
            SessionCmd::RejectEnterWorld(code) => {
                self.session.reject_enter_world(code, "").map(|()| None)
            }
            SessionCmd::RejectChat(code) => {
                let r = self.session.reject_chat(code, "", now);
                // The answer goes out first, then what was held behind its ChatSend.
                self.drain(false);
                self.chat_answered();
                r
            }
            SessionCmd::ChatDelivered => {
                self.chat_answered();
                Ok(None)
            }
            SessionCmd::ChannelJoined(name) => {
                let r = self.session.channel_joined(name).map(|()| None);
                self.drain(false);
                self.chat_answered();
                r
            }
            SessionCmd::ChannelLeft(name) => {
                let r = self
                    .session
                    .channel_left(name, ChannelLeftReason::Left)
                    .map(|()| None);
                self.drain(false);
                self.chat_answered();
                r
            }
            SessionCmd::RefuseChannel { code, name } => {
                let r = self.session.refuse_channel(code, &name, "", now);
                self.drain(false);
                self.chat_answered();
                r
            }
            SessionCmd::Disconnect(code, text) => Ok(self.session.disconnect(code, &text, now)),
        };
        match result {
            Ok(Some(e)) => self.map_event(e, now, &mut out),
            Ok(None) => {}
            Err(e) => tracing::debug!(error = %e, "simulation request dropped"),
        }
        self.drain(false);
        out
    }
}
