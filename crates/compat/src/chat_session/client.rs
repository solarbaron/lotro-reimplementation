// SPDX-License-Identifier: GPL-3.0-or-later
//! The chat-server session driver (`docs/specs/official-messages-play.md` §6.6) and its
//! mapping to `tessera_net`'s neutral events and commands (§8).
//!
//! [`ChatClient`] owns its own session multiplexer and socket, separate from the
//! world-server connection (`official-session.md` §3.4), so a chat-server timeout or
//! reconnect never touches the world session (§11 item 2). The live socket is made only
//! by [`ChatClient::open`], which needs a [`NetworkAccess`] (`official-entry.md` §6.1);
//! [`ChatClient::with_transport`] takes a transport made elsewhere (for tests, the
//! in-memory transport of the session module).

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::Arc;

use tessera_net::model::{MAX_CHAT_LEN, MAX_NAME_LEN, is_valid_name, sanitize_text};
use tessera_net::{ChatChannel, Command, Event, SocialCommand, SocialEvent};

use crate::chat_session::events::{ChatEvent, ChatRequest};
use crate::chat_session::state::{ChatAction, ChatPhase, ChatSessionState};
use crate::chat_session::wire::{
    ChatMessagePayload, ChatRoomDescriptor, DescriptorKind, MSG_TYPE_CHAT,
};
use crate::chat_session::{CHAT_RECIPIENT_ID, chat_auth_header};
use crate::net_access::{NetworkAccess, StdUdpTransport};
use crate::play::PlayError;
use crate::play::chat::{MAX_ARGUMENT_UNITS, MAX_CHAT_UNITS};
use crate::play::check_units;
use crate::session::{
    BitTreeCodec, MAX_DATAGRAM_BUF, OptionalHeaders, SessionAuthHeader, SessionEvent,
    SessionMultiplexer, UdpDatagramTransport, split_blob,
};
use crate::world::{PropertySchemaLookup, decode_blob_payload, encode_blob_payload};

/// Most rooms kept in the directory (engineering cap against a hostile server).
pub const MAX_ROOMS: usize = 256;
/// Most requests awaiting a response (engineering cap).
pub const MAX_PENDING: usize = 256;
/// Most datagrams read per [`ChatClient::poll`] (engineering cap).
pub const MAX_DATAGRAMS_PER_POLL: usize = 256;
/// Delivery queue of outgoing chat-server blobs. The spec does not say which queue the
/// chat server uses; queue 2 (`ReliableOrdered`) is an engineering choice.
pub const CHAT_QUEUE_ID: u8 = 2;

/// What the chat client needs to connect: the endpoint (from the launch parameters,
/// §6.1.1) and the credentials of §6.6.2. The account and ticket are kept out of
/// `Debug`.
#[derive(Clone)]
pub struct ChatConfig {
    /// The chat-server endpoint.
    pub endpoint: SocketAddrV4,
    account: String,
    ticket_base64: Option<String>,
    /// Unix time in seconds when the transport was initialised (`clientTimestamp`).
    pub client_timestamp: u32,
}

impl std::fmt::Debug for ChatConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChatConfig")
            .field("endpoint", &self.endpoint)
            .field("account", &"<redacted>")
            .field("ticket", &self.ticket_base64.as_ref().map(|_| "<redacted>"))
            .field("client_timestamp", &self.client_timestamp)
            .finish()
    }
}

impl ChatConfig {
    /// A configuration for `endpoint`, the `--account` value and the current GLS ticket
    /// (Base64, as the launcher passes it), if any.
    #[must_use]
    pub fn new(
        endpoint: SocketAddrV4,
        account: impl Into<String>,
        ticket_base64: Option<String>,
        client_timestamp: u32,
    ) -> Self {
        Self {
            endpoint,
            account: account.into(),
            ticket_base64,
            client_timestamp,
        }
    }

    /// Replaces the ticket after an in-game refresh (`official-login.md` §4.3); the next
    /// connection attempt uses it (§6.1.2).
    pub fn set_ticket(&mut self, ticket_base64: Option<String>) {
        self.ticket_base64 = ticket_base64;
    }

    fn auth_header(&self) -> Result<SessionAuthHeader, PlayError> {
        chat_auth_header(
            &self.account,
            self.ticket_base64.as_deref(),
            self.client_timestamp,
        )
    }
}

/// Diagnostics of the chat client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChatCounters {
    /// Admin responses (`0x15F`) received and ignored (decision 0042 item 2).
    pub admin_ignored: u64,
    /// Chat events that failed to decode.
    pub decode_errors: u64,
    /// Datagrams the session layer refused.
    pub transport_errors: u64,
    /// Connection attempts made.
    pub connects: u64,
    /// Heartbeats sent.
    pub heartbeats: u64,
    /// Room messages that could not be mapped (unknown room, other message types).
    pub unmapped_messages: u64,
}

/// What one [`ChatClient::poll`] produced.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ChatPoll {
    /// Neutral events.
    pub events: Vec<Event>,
    /// Every decoded chat event, for the connector.
    pub raw: Vec<ChatEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Pending {
    Join(String),
    Leave(String),
    Other,
}

/// The chat-server session (§6.6).
pub struct ChatClient<T: UdpDatagramTransport> {
    transport: T,
    config: ChatConfig,
    codec: Option<Arc<dyn BitTreeCodec>>,
    mux: SessionMultiplexer,
    conn: Option<u16>,
    state: ChatSessionState,
    next_ctx: u32,
    pending: BTreeMap<u32, Pending>,
    rooms: Vec<ChatRoomDescriptor>,
    self_desc: ChatRoomDescriptor,
    fellowship_channel_type: Option<u32>,
    order_num: u16,
    blob_id: u16,
    counters: ChatCounters,
    recv_buf: Vec<u8>,
}

impl<T: UdpDatagramTransport> std::fmt::Debug for ChatClient<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChatClient")
            .field("config", &self.config)
            .field("phase", &self.state.phase())
            .field("rooms", &self.rooms.len())
            .field("counters", &self.counters)
            .finish_non_exhaustive()
    }
}

impl ChatClient<StdUdpTransport> {
    /// Opens the chat-server socket through `access` and prepares the session; the first
    /// connection attempt follows after the initial delay of §6.6.3.
    ///
    /// # Errors
    ///
    /// [`PlayError`] when the ticket is not valid Base64 or binding fails.
    pub fn open(
        access: &NetworkAccess,
        config: ChatConfig,
        codec: Option<Arc<dyn BitTreeCodec>>,
        now: f64,
    ) -> Result<Self, PlayError> {
        let transport = access.bind_udp(Ipv4Addr::UNSPECIFIED, 0, false)?;
        Self::with_transport(transport, config, codec, now)
    }
}

impl<T: UdpDatagramTransport> ChatClient<T> {
    /// A chat client over `transport`. `codec` decodes bit-tree encoded datagrams
    /// (`official-session.md` §4.1.2), from the install's tables.
    ///
    /// # Errors
    ///
    /// [`PlayError`] when the ticket is not valid Base64.
    pub fn with_transport(
        transport: T,
        config: ChatConfig,
        codec: Option<Arc<dyn BitTreeCodec>>,
        now: f64,
    ) -> Result<Self, PlayError> {
        config.auth_header()?;
        Ok(Self {
            transport,
            config,
            codec,
            mux: SessionMultiplexer::new(),
            conn: None,
            state: ChatSessionState::new(now),
            next_ctx: 1,
            pending: BTreeMap::new(),
            rooms: Vec::new(),
            self_desc: ChatRoomDescriptor::Null,
            fellowship_channel_type: None,
            order_num: 0,
            blob_id: 1,
            counters: ChatCounters::default(),
            recv_buf: vec![0u8; MAX_DATAGRAM_BUF],
        })
    }

    /// The transport (for tests: the in-memory transport's queues).
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    /// The configuration (to refresh the ticket).
    pub fn config_mut(&mut self) -> &mut ChatConfig {
        &mut self.config
    }

    /// Whether the chat server accepted the login.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.state.is_ready()
    }

    /// The session phase.
    #[must_use]
    pub fn phase(&self) -> ChatPhase {
        self.state.phase()
    }

    /// The rooms the server added (`0x18B`) or the client entered.
    #[must_use]
    pub fn rooms(&self) -> &[ChatRoomDescriptor] {
        &self.rooms
    }

    /// The counters.
    #[must_use]
    pub fn counters(&self) -> ChatCounters {
        self.counters
    }

    /// The local player's descriptor (`desc_type = 1`), the sender of outgoing messages.
    pub fn set_self_descriptor(&mut self, entity_id: u64, name: &str) {
        self.self_desc = ChatRoomDescriptor::new(DescriptorKind::Entity, entity_id, name);
    }

    /// The server-room `channel_type` (enum `0x2300003B`, §6.7) of the fellowship room,
    /// from the install's data; rooms of this type map to [`ChatChannel::Fellowship`].
    pub fn set_fellowship_channel_type(&mut self, channel_type: Option<u32>) {
        self.fellowship_channel_type = channel_type;
    }

    fn teardown(&mut self) {
        self.mux = SessionMultiplexer::new();
        self.conn = None;
        self.pending.clear();
    }

    fn connect(&mut self, now: f64) -> Result<(), PlayError> {
        self.teardown();
        let id = self.mux.create_connection(self.config.endpoint, now)?;
        let auth = self.config.auth_header()?;
        if let Some(conn) = self.mux.connection_mut(id) {
            conn.start_login(auth, now)?;
        }
        self.conn = Some(id);
        self.order_num = 0;
        self.counters.connects += 1;
        Ok(())
    }

    fn send_raw(&mut self, req: &ChatRequest, now: f64) -> Result<(), PlayError> {
        let id = self.conn.ok_or(PlayError::ChatNotReady)?;
        let inner = req.encode()?;
        let payload = encode_blob_payload(&inner);
        let blob_id = self.blob_id;
        self.blob_id = self.blob_id.checked_add(1).unwrap_or(1);
        let frags = split_blob(
            CHAT_QUEUE_ID,
            self.order_num,
            blob_id,
            0,
            Some(CHAT_RECIPIENT_ID),
            req.opcode(),
            &payload,
            false,
            None,
        )?;
        self.order_num = self.order_num.wrapping_add(1);
        let codec = self.codec.clone();
        let conn = self.mux.connection_mut(id).ok_or(PlayError::ChatNotReady)?;
        for frag in frags {
            conn.send_sequenced_packet(
                OptionalHeaders::default(),
                vec![frag],
                true,
                now,
                codec.as_deref(),
            )?;
        }
        Ok(())
    }

    fn remember(&mut self, ctx: u32, pending: Pending) {
        if self.pending.len() >= MAX_PENDING
            && let Some(first) = self.pending.keys().next().copied()
        {
            self.pending.remove(&first);
        }
        self.pending.insert(ctx, pending);
    }

    /// Sends `req` with a fresh `ctx_id`, returned for requests that carry one.
    ///
    /// # Errors
    ///
    /// [`PlayError::ChatNotReady`] before a successful login, or an encoding or session
    /// error.
    pub fn send(&mut self, req: ChatRequest, now: f64) -> Result<Option<u32>, PlayError> {
        self.send_tracked(req, Pending::Other, now)
    }

    fn send_tracked(
        &mut self,
        mut req: ChatRequest,
        pending: Pending,
        now: f64,
    ) -> Result<Option<u32>, PlayError> {
        if !self.state.is_ready() {
            return Err(PlayError::ChatNotReady);
        }
        let ctx = req.ctx_id().map(|_| {
            let c = self.next_ctx;
            self.next_ctx = self.next_ctx.checked_add(1).unwrap_or(1);
            c
        });
        if let Some(c) = ctx {
            req.set_ctx_id(c);
        }
        self.send_raw(&req, now)?;
        if let Some(c) = ctx {
            self.remember(c, pending);
        }
        Ok(ctx)
    }

    fn room_by_name(&self, name: &str) -> Option<&ChatRoomDescriptor> {
        self.rooms.iter().find(|r| r.name() == Some(name))
    }

    /// Maps a neutral command to chat-server requests (§8): `Chat` on `Named` and
    /// `Fellowship` channels (`0x16A`), `Social::JoinChannel` (`0x177`) and
    /// `Social::LeaveChannel` (`0x18E`).
    ///
    /// # Errors
    ///
    /// [`PlayError`] for other commands, an unknown room, text over the limits of §6.2,
    /// or before login.
    pub fn command(&mut self, cmd: &Command, now: f64) -> Result<(), PlayError> {
        match cmd {
            Command::Chat { channel, text } => {
                if text.is_empty() {
                    return Err(PlayError::EmptyText);
                }
                check_units(text, MAX_CHAT_UNITS)?;
                let room = match channel {
                    ChatChannel::Named { name } => self
                        .room_by_name(name)
                        .cloned()
                        .ok_or_else(|| PlayError::UnknownRoom(name.clone()))?,
                    ChatChannel::Fellowship => {
                        let t = self
                            .fellowship_channel_type
                            .ok_or(PlayError::UnmappedChannel("fellowship"))?;
                        self.rooms
                            .iter()
                            .find(|r| r.server_room_types().map(|(c, _)| c) == Some(t))
                            .cloned()
                            .ok_or(PlayError::UnmappedChannel("fellowship"))?
                    }
                    _ => return Err(PlayError::UnsupportedCommand("world-server chat channel")),
                };
                let data =
                    ChatMessagePayload::text(self.self_desc.clone(), text.clone()).to_bytes()?;
                let req = ChatRequest::SendDataToRoom {
                    ctx_id: 0,
                    sender: self.self_desc.clone(),
                    room,
                    msg_type: MSG_TYPE_CHAT,
                    data,
                    delivery_flag: false,
                };
                self.send(req, now).map(|_| ())
            }
            Command::Social(SocialCommand::JoinChannel { name }) => {
                check_units(name, MAX_ARGUMENT_UNITS)?;
                let req = ChatRequest::RoomEnter {
                    ctx_id: 0,
                    entity: self.self_desc.clone(),
                    room: ChatRoomDescriptor::new(DescriptorKind::UserRoom, 0, name.clone()),
                    flags_a: 0,
                    flags_b: 0,
                };
                self.send_tracked(req, Pending::Join(name.clone()), now)
                    .map(|_| ())
            }
            Command::Social(SocialCommand::LeaveChannel { name }) => {
                check_units(name, MAX_ARGUMENT_UNITS)?;
                let room = self.room_by_name(name).cloned().unwrap_or_else(|| {
                    ChatRoomDescriptor::new(DescriptorKind::UserRoom, 0, name.clone())
                });
                let req = ChatRequest::RoomLeave {
                    ctx_id: 0,
                    entity: self.self_desc.clone(),
                    room,
                };
                self.send_tracked(req, Pending::Leave(name.clone()), now)
                    .map(|_| ())
            }
            other => Err(PlayError::UnsupportedCommand(other.name())),
        }
    }

    /// Closes the session: a graceful transport disconnect, no reconnection.
    pub fn close(&mut self, now: f64) {
        self.state.close();
        if let Some(conn) = self.conn.and_then(|id| self.mux.connection_mut(id)) {
            conn.request_disconnect(now);
        }
    }

    /// Reads the socket, runs the session and the state machine of §6.6.2–§6.6.3, and
    /// returns what happened. `schema` types the property tables some chat payloads carry.
    ///
    /// # Errors
    ///
    /// [`PlayError::Session`] when the transport fails. Malformed input is counted, not
    /// returned.
    pub fn poll(
        &mut self,
        now: f64,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<ChatPoll, PlayError> {
        let mut out = ChatPoll::default();
        let mut buf = std::mem::take(&mut self.recv_buf);
        buf.resize(MAX_DATAGRAM_BUF, 0);
        for _ in 0..MAX_DATAGRAMS_PER_POLL {
            let received = match self.transport.recv_from(&mut buf) {
                Ok(r) => r,
                Err(e) => {
                    self.recv_buf = buf;
                    return Err(e.into());
                }
            };
            let Some((len, src)) = received else {
                break;
            };
            let Some(datagram) = buf.get(..len) else {
                break;
            };
            if self.conn.is_none() {
                continue;
            }
            match self
                .mux
                .receive_from_peer(datagram, src, now, self.codec.as_deref())
            {
                Ok((id, _)) => self.conn = Some(id),
                Err(e) => {
                    self.counters.transport_errors += 1;
                    tracing::debug!(error = %e, "chat-server datagram refused");
                }
            }
        }
        self.recv_buf = buf;
        self.drain_session(now, schema, &mut out);
        if let Some(conn) = self.conn.and_then(|id| self.mux.connection_mut(id))
            && let Err(e) = conn.tick(now, self.codec.as_deref())
        {
            tracing::debug!(error = %e, "chat-server session tick failed");
        }
        self.drain_session(now, schema, &mut out);
        if let Some(action) = self.state.tick(now) {
            self.act(action, now)?;
        }
        self.mux.flush_outbound(&mut self.transport)?;
        Ok(out)
    }

    fn act(&mut self, action: ChatAction, now: f64) -> Result<(), PlayError> {
        match action {
            ChatAction::Connect => self.connect(now),
            ChatAction::SendHeartbeat => {
                self.counters.heartbeats += 1;
                self.send_raw(&ChatRequest::Heartbeat, now)
            }
            ChatAction::Disconnect => {
                self.teardown();
                Ok(())
            }
        }
    }

    fn drain_session(&mut self, now: f64, schema: &dyn PropertySchemaLookup, out: &mut ChatPoll) {
        let mut session_events = Vec::new();
        if let Some(conn) = self.conn.and_then(|id| self.mux.connection_mut(id)) {
            while let Some(ev) = conn.pop_event() {
                session_events.push(ev);
            }
        }
        for ev in session_events {
            match ev {
                SessionEvent::Connected { .. } => self.state.on_transport_connected(now),
                SessionEvent::Disconnected { .. } => {
                    self.state.on_transport_lost(now);
                    self.teardown();
                    return;
                }
                SessionEvent::BlobReceived(blob) => {
                    let decoded = decode_blob_payload(&blob.payload, blob.recipient_id)
                        .map_err(PlayError::from)
                        .and_then(|d| ChatEvent::decode(blob.blob_header_word, &d.inner_payload));
                    match decoded {
                        Ok(ev) => {
                            self.state.on_event(now);
                            self.on_chat_event(&ev, now, schema, out);
                            out.raw.push(ev);
                        }
                        Err(e) => {
                            self.counters.decode_errors += 1;
                            tracing::debug!(error = %e, "chat-server event refused");
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn add_room(&mut self, room: ChatRoomDescriptor) {
        if matches!(room, ChatRoomDescriptor::Null) {
            return;
        }
        self.rooms.retain(|r| {
            !(r.kind() == room.kind() && r.id() == room.id() && r.name() == room.name())
        });
        if self.rooms.len() >= MAX_ROOMS {
            self.rooms.remove(0);
        }
        self.rooms.push(room);
    }

    fn remove_room_named(&mut self, name: &str) {
        self.rooms.retain(|r| r.name() != Some(name));
    }

    fn channel_name(name: &str) -> Option<String> {
        let n = sanitize_text(name.trim(), MAX_NAME_LEN).into_owned();
        is_valid_name(&n).then_some(n)
    }

    fn joined(name: &str, out: &mut ChatPoll) {
        if let Some(channel) = Self::channel_name(name) {
            out.events
                .push(Event::Social(SocialEvent::ChannelJoined { channel }));
        }
    }

    fn left(name: &str, out: &mut ChatPoll) {
        if let Some(channel) = Self::channel_name(name) {
            out.events
                .push(Event::Social(SocialEvent::ChannelLeft { channel }));
        }
    }

    fn room_channel(&self, room: &ChatRoomDescriptor) -> Option<ChatChannel> {
        if let (Some(t), Some((c, _))) = (self.fellowship_channel_type, room.server_room_types())
            && t == c
        {
            return Some(ChatChannel::Fellowship);
        }
        let name = room.name().filter(|n| !n.is_empty()).or_else(|| {
            self.rooms
                .iter()
                .find(|r| r.id() == room.id() && r.kind() == room.kind())
                .and_then(ChatRoomDescriptor::name)
        })?;
        Self::channel_name(name).map(|name| ChatChannel::Named { name })
    }

    fn on_chat_event(
        &mut self,
        ev: &ChatEvent,
        now: f64,
        schema: &dyn PropertySchemaLookup,
        out: &mut ChatPoll,
    ) {
        match ev {
            ChatEvent::LoginStatus { status } => {
                if self.state.on_login_status(*status, now) == Some(ChatAction::Disconnect) {
                    self.teardown();
                }
            }
            ChatEvent::AdminIgnored { .. } => self.counters.admin_ignored += 1,
            ChatEvent::AddRoom { room, .. } => {
                if let Some(name) = room.name() {
                    Self::joined(name, out);
                }
                self.add_room(room.clone());
            }
            ChatEvent::RemoveRoom { room, .. } => {
                if let Some(name) = room.name() {
                    let name = name.to_owned();
                    self.remove_room_named(&name);
                    Self::left(&name, out);
                }
            }
            ChatEvent::RoomClosed { room_name, .. } => {
                self.remove_room_named(room_name);
                Self::left(room_name, out);
            }
            ChatEvent::RoomEnterResp {
                ctx_id,
                status,
                room_name,
                room,
            } => {
                let asked = self.pending.remove(ctx_id);
                let name = if room_name.is_empty() {
                    match &asked {
                        Some(Pending::Join(n)) => n.clone(),
                        _ => String::new(),
                    }
                } else {
                    room_name.clone()
                };
                if *status >= 0 {
                    let desc = match room {
                        ChatRoomDescriptor::Null => {
                            ChatRoomDescriptor::new(DescriptorKind::UserRoom, 0, name.clone())
                        }
                        other => other.clone(),
                    };
                    self.add_room(desc);
                    Self::joined(&name, out);
                } else {
                    out.events.push(Event::ChatMessage {
                        channel: ChatChannel::System,
                        sender: None,
                        text: sanitize_text(
                            &format!("Could not join the channel {name} (status {status})."),
                            MAX_CHAT_LEN,
                        )
                        .into_owned(),
                    });
                }
            }
            ChatEvent::CreateRoomResp {
                ctx_id,
                status,
                room_name,
            } => {
                self.pending.remove(ctx_id);
                if *status >= 0 {
                    self.add_room(ChatRoomDescriptor::new(
                        DescriptorKind::UserRoom,
                        0,
                        room_name.clone(),
                    ));
                    Self::joined(room_name, out);
                }
            }
            ChatEvent::StatusWords {
                opcode: 0x160,
                ctx_id,
                status,
                ..
            }
            | ChatEvent::Status {
                opcode: 0x166,
                ctx_id,
                status,
            } => {
                if let Some(Pending::Leave(name)) = self.pending.remove(ctx_id)
                    && *status >= 0
                {
                    self.remove_room_named(&name);
                    Self::left(&name, out);
                }
            }
            ChatEvent::RecvDataFromRoom {
                sender,
                room,
                msg_type,
                data,
                ..
            } => {
                let Some(channel) = self.room_channel(room) else {
                    self.counters.unmapped_messages += 1;
                    return;
                };
                self.push_message(channel, sender, *msg_type, data, schema, out);
            }
            ChatEvent::RecvDataFromEntity {
                sender,
                msg_type,
                data,
                ..
            } => {
                let Some(peer) = sender.name().and_then(Self::channel_name) else {
                    self.counters.unmapped_messages += 1;
                    return;
                };
                self.push_message(
                    ChatChannel::Tell { peer },
                    sender,
                    *msg_type,
                    data,
                    schema,
                    out,
                );
            }
            ev => {
                if let Some(ctx) = ev.ctx_id() {
                    self.pending.remove(&ctx);
                }
            }
        }
    }

    fn push_message(
        &mut self,
        channel: ChatChannel,
        sender: &ChatRoomDescriptor,
        msg_type: u32,
        data: &[u8],
        schema: &dyn PropertySchemaLookup,
        out: &mut ChatPoll,
    ) {
        if msg_type != MSG_TYPE_CHAT {
            self.counters.unmapped_messages += 1;
            return;
        }
        let Ok(payload) = ChatMessagePayload::from_bytes(data, schema) else {
            self.counters.decode_errors += 1;
            return;
        };
        let name = payload
            .sender
            .name()
            .filter(|n| !n.is_empty())
            .or_else(|| sender.name())
            .map(|n| sanitize_text(n.trim(), MAX_NAME_LEN).into_owned())
            .filter(|n| !n.is_empty());
        out.events.push(Event::ChatMessage {
            channel,
            sender: name,
            text: sanitize_text(&payload.text, MAX_CHAT_LEN).into_owned(),
        });
    }
}
