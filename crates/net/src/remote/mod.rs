// SPDX-License-Identifier: GPL-3.0-or-later
//! The test-server backend: a [`Backend`] that connects to a `tessera-server` over the
//! Tessera protocol (`docs/specs/tessera-protocol.md`, decision 0030), so [`Session`]
//! works with a real server exactly as it does with the sandbox.
//!
//! This is Tessera's **own** protocol to Tessera's own server. It has no relation to any
//! official service (decision 0004).
//!
//! # Threads
//!
//! [`RemoteBackend`] lives on the main thread. On [`Command::Connect`] it starts a session
//! task on the process's one network thread, which runs a tokio `current_thread` runtime
//! and one quinn client endpoint per address family, both shared by every backend
//! (ARCH §19.4). The two talk only over bounded channels:
//! [`Backend::poll`] drains decoded network input without waiting, and
//! [`Backend::submit`] queues commands without waiting (a full queue is
//! [`NetError::SendQueueFull`]). Moves go through a single "newest move" slot, so they can
//! never fill the queue. Everything from the network is validated by `tessera-protocol`'s
//! decoders and client state machine before it becomes an [`Event`].
//!
//! # What the backend does
//!
//! - **Handshake** (§4): QUIC v1 + TLS 1.3, ALPN `tessera/1`, the server's Ed25519 key
//!   checked against the pin in the connect string or the known-servers file (trust on
//!   first use only when [`FirstUse::Accept`] is configured), then `Hello`, then `Login`
//!   (or `Register`) with the account and [`Password`] the caller supplied. The password
//!   is never logged and goes only to a server whose key was verified: pinned, found in
//!   the known-servers file, or explicitly accepted on first use. A known-servers file
//!   that exists but cannot be read fails the connection as `KEY_UNKNOWN`.
//! - **Events** (Appendix A): `LoginOk` → [`Event::Connected`]; `WorldEnter` →
//!   [`Event::WorldEntered`], [`Event::AppearanceChanged`], [`Event::LocationReport`];
//!   entities → [`Event::EntityCreated`] (with appearance), [`Event::EntityMoved`] (with
//!   velocity), [`Event::AppearanceChanged`], [`Event::EntityDespawned`]; chat, notices
//!   and refusals → [`Event::ChatMessage`]; every end → one [`Event::Disconnected`] with
//!   a reason ([`RemoteBackend::failure`] has the details).
//! - **Named chat channels** (protocol minor 2, decision 0035, `scenario-feed.md` §9.8):
//!   only when [`RemoteBackend::has_chat_channels`] is true (negotiated minor ≥ 2 and the
//!   server's `CHAT_CHANNELS`), `JoinChannel`, `LeaveChannel` and `Named` chat lines are
//!   sent; otherwise they are refused with [`NetError::InvalidCommand`] and nothing is
//!   sent. `ChannelJoined` and `ChannelLeft` surface as [`Event::Social`]; a removal
//!   (`ChannelLeft` reason `REMOVED`) then posts a System chat line, and a
//!   `ChannelRefused` posts only a System line in Tessera's own words (the server's text
//!   is logged).
//! - **Movement** (§8.2): [`Command::Move`] updates the newest-move slot; the network
//!   thread sends `MoveInput` at 15 Hz while moving, at once on gait changes, start and
//!   stop, and as a 1 Hz heartbeat at rest. Moves the client sent before it saw a
//!   teleport or correction are not sent (§8.4).
//! - **Statics are not replicated** (§9.1): with a client-supplied
//!   [`crate::sandbox::WorldSource`] the backend streams the statics of the
//!   player's neighbourhood from the client's own data ([`Event::StaticSpawned`],
//!   [`Event::StaticsCleared`]), with the sandbox's streaming code.
//! - **Selection is local** (`scenario-feed.md` §9.7, decision 0032 D1):
//!   [`CombatCommand::SelectTarget`](crate::CombatCommand) with a known target is answered
//!   by an [`Event::Combat`] `TargetChanged` for the player first in the next poll; nothing
//!   is sent to the server. `CreateCharacter` is sent when the server allows creation
//!   (`character-screens.md` §7.1.1); every other new gameplay command is refused with
//!   [`NetError::InvalidCommand`].
//! - **Interpolation** (§8.6): every remote entity has a snapshot buffer;
//!   [`RemoteBackend::sample`] gives its state at the current render time, using the
//!   clock-offset estimate (§11.2). [`RemoteBackend::stats`] has the transport and
//!   clock statistics for a HUD or the load tools.
//!
//! ```no_run
//! use std::time::Instant;
//! use tessera_net::{Command, Event, Session};
//! use tessera_net::remote::{Password, RemoteBackend, RemoteConfig};
//!
//! let config = RemoteConfig::new(
//!     "tessera://192.0.2.10:47000?pin=sha256:06e3fd8fda29bb60ab59557de61edb0aecdb231134be30e75b455f8e1b792fa9",
//!     "alice",
//!     Password::new("correct horse"),
//! )?;
//! let mut session = Session::new(RemoteBackend::new(config)?);
//! session.send(Instant::now(), Command::Connect)?;
//! let mut events = Vec::new();
//! session.poll_into(Instant::now(), &mut events);
//! # Ok::<(), tessera_net::NetError>(())
//! ```

mod config;
mod convert;
mod driver;
mod harness;
mod known;
mod link;
mod mapper;
mod stats;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tessera_protocol::message::Credentials;
pub use tessera_protocol::message::ServerCaps;
use tessera_protocol::types::Motion;
use tessera_protocol::value::Stance as WireStance;
use tokio::sync::{mpsc, watch};

pub use config::{
    COMMAND_CHANNEL_CAPACITY, EVENT_CHANNEL_CAPACITY, FirstUse, MAX_CONTROL_BACKLOG, PENDING_PAUSE,
    Password, READ_CHUNK, RemoteConfig, Trust,
};
pub use harness::{HarnessInput, InboundHarness, MAX_ADVANCE, MAX_RECORDED_SENDS};
pub use known::{MAX_KNOWN_SERVERS_BYTES, load_known_servers, record_known_server};
pub use mapper::{EntityState, LOCAL_ID_BASE};
pub use stats::{RemoteFailure, RemoteStats, ServerInfo};
pub use tessera_protocol::client::ClientConfig;
pub use tessera_protocol::codes::{
    AppErrorCode, ClientFailure, EnterWorldFailedCode, LoginFailedCode,
};
pub use tessera_protocol::connect::{ConnectString, ServerKey};
pub use tessera_protocol::fingerprint::Fingerprint;
pub use tessera_protocol::message::WorldClock;
pub use tessera_protocol::types::Gait;
pub use tessera_protocol::value::{Emote, Stance};

use self::driver::{Driver, Inbound, MoveSlot, NetCommand};
use self::mapper::Mapper;
use crate::domain::{CharacterCommand, CombatCommand, SocialCommand};
use crate::interp::{Sample, render_time};
use crate::model::{Appearance, EntityId, Vec3};
use crate::sandbox::world::{StaticStats, WorldSource};
use crate::session::{Backend, Command, Event};
use crate::{NetError, Session};

/// Why a channel command is refused below minor 2 or without `CHAT_CHANNELS`.
const CHANNELS_UNAVAILABLE: &str = "chat channels need protocol minor 2 with CHAT_CHANNELS";

/// What a [`Command`] becomes.
enum Routed {
    Connect,
    Net(NetCommand),
    Move(MoveSlot),
    Disconnect,
    /// Answered on the main thread; nothing goes to the network thread
    /// (`SelectTarget`, `scenario-feed.md` §9.7, decision 0032 D1).
    Local,
}

/// Turns a command into network work, validating what `Session` cannot (the protocol's own
/// limits).
fn route(cmd: Command, mapper: &mut Mapper) -> Result<Routed, NetError> {
    Ok(match cmd {
        Command::Connect => Routed::Connect,
        Command::EnterWorld { character } => {
            if mapper.outstanding_create.is_some() {
                // A second outstanding request is fatal on the wire (§13.1).
                return Err(NetError::InvalidCommand(
                    "a character creation is still outstanding",
                ));
            }
            Routed::Net(NetCommand::EnterWorld(character))
        }
        Command::Character(CharacterCommand::CreateCharacter {
            name, appearance, ..
        }) => {
            // character-screens.md §7.1.1 step 2 (Session checked the state and §9.5's
            // validation); class and nationality are dropped (server-content.md §7).
            let caps = mapper.server.as_ref().map_or(ServerCaps::NONE, |s| s.caps);
            if !caps.contains(ServerCaps::CHARACTER_CREATION) {
                return Err(NetError::InvalidCommand(
                    "this server does not allow creating characters",
                ));
            }
            let Some(appearance) = appearance else {
                return Err(NetError::InvalidCommand(
                    "creating a character on the test server needs an appearance",
                ));
            };
            if mapper.outstanding_create.is_some() {
                return Err(NetError::InvalidCommand(
                    "a character creation is still outstanding",
                ));
            }
            let wire = convert::wire_appearance(&appearance)
                .ok_or(NetError::InvalidCommand("appearance cannot be sent"))?;
            // The look travels only at minor ≥ 1; the protocol encoder leaves it off at
            // minor 0 (scenario-feed.md §9.7).
            let look = convert::wire_look(appearance.look.as_deref());
            if look.check().is_err() {
                return Err(NetError::InvalidCommand("look cannot be sent"));
            }
            // The name becomes the outstanding creation only once the command is queued
            // for the network thread ([`NetCommand::creation`]; §7.1.1 step 2: a refused
            // command records nothing).
            Routed::Net(NetCommand::CreateCharacter {
                name,
                appearance: Box::new(wire),
                look,
            })
        }
        Command::Move {
            location,
            heading,
            velocity,
        } => {
            mapper.player_moved(location, heading);
            let state = mapper.player_state();
            Routed::Move(MoveSlot {
                epoch: mapper.epoch,
                location: convert::wire_location(&location),
                heading,
                velocity: convert::wire_vec3(velocity),
                motion: derive_motion(velocity, heading, state.walk_speed, state.run_speed),
            })
        }
        Command::Teleport { destination } => Routed::Net(NetCommand::Teleport(
            convert::wire_destination(&destination),
        )),
        Command::QueryLocation => Routed::Net(NetCommand::QueryLocation),
        Command::Chat { channel, text } => {
            let wire = convert::wire_chat_channel(&channel, mapper.chat_channels()).ok_or(
                NetError::InvalidCommand(match channel {
                    crate::ChatChannel::System => "cannot send on the system channel",
                    crate::ChatChannel::Named { .. } => CHANNELS_UNAVAILABLE,
                    _ => "chat channel has no wire form on the test server",
                }),
            )?;
            Routed::Net(NetCommand::Chat(wire, text))
        }
        // Named channels at minor ≥ 2 with CHAT_CHANNELS (scenario-feed.md §9.8,
        // decision 0035 D1); refused below it, nothing sent.
        Command::Social(SocialCommand::JoinChannel { name }) => {
            if !mapper.chat_channels() {
                return Err(NetError::InvalidCommand(CHANNELS_UNAVAILABLE));
            }
            Routed::Net(NetCommand::JoinChannel(name))
        }
        Command::Social(SocialCommand::LeaveChannel { name }) => {
            if !mapper.chat_channels() {
                return Err(NetError::InvalidCommand(CHANNELS_UNAVAILABLE));
            }
            Routed::Net(NetCommand::LeaveChannel(name))
        }
        Command::Disconnect => Routed::Disconnect,
        // Selection is local: the backend answers with `TargetChanged` for the player in
        // the next poll and sends nothing (scenario-feed.md §9.7, decision 0032 D1).
        Command::Combat(CombatCommand::SelectTarget { target }) => {
            mapper.select_target(target)?;
            Routed::Local
        }
        // The test server has no gameplay (LEGAL.md §11.1 rule 5; scenario-feed.md §9.5,
        // §9.7, §9.8): DeleteCharacter, every other Social command (fellowship, trade, lists,
        // performances) and every other new command are refused.
        Command::Character(_)
        | Command::Inventory(_)
        | Command::Combat(_)
        | Command::Npc(_)
        | Command::Social(_)
        | Command::World(_)
        | Command::Progression(_) => {
            return Err(NetError::InvalidCommand(
                "gameplay commands are not available on the test server",
            ));
        }
    })
}

/// The motion byte for a move (§8.1). `Command::Move` carries no ground state, so the gait
/// is never Airborne here (see the spec's OPEN note); walk and run come from the
/// horizontal speed and the player's `WALK_SPEED`/`RUN_SPEED`.
fn derive_motion(velocity: Vec3, heading: f32, walk: f32, run: f32) -> Motion {
    let horizontal = velocity.x.hypot(velocity.y);
    let forward = velocity.x * heading.cos() + velocity.y * heading.sin();
    let backward = horizontal >= tessera_protocol::limits::STANDING_SPEED && forward < -1e-3;
    Motion::derive(true, horizontal, walk, run, backward)
}

/// Commands beyond the neutral [`Command`] set, for a client or tool that knows it talks
/// to a Tessera server: emotes, stance and appearance changes (`tessera-protocol.md`
/// §6.2.15–§6.2.17). Cheap to clone and `Send`; every call is non-blocking.
#[derive(Debug, Clone)]
pub struct RemoteHandle {
    commands: mpsc::Sender<NetCommand>,
}

impl RemoteHandle {
    fn send(&self, cmd: NetCommand) -> Result<(), NetError> {
        self.commands.try_send(cmd).map_err(|e| match e {
            mpsc::error::TrySendError::Full(_) => NetError::SendQueueFull,
            mpsc::error::TrySendError::Closed(_) => NetError::Closed,
        })
    }

    /// Plays an emote: `anim` is an opaque animation key from the client's data, 0 stops
    /// the current emote (`EmoteSet`).
    ///
    /// # Errors
    ///
    /// [`NetError::SendQueueFull`] or [`NetError::Closed`].
    pub fn emote(&self, anim: u32, looping: bool) -> Result<(), NetError> {
        self.send(NetCommand::Emote { anim, looping })
    }

    /// Sets the player's stance (`StanceSet`).
    ///
    /// # Errors
    ///
    /// [`NetError::SendQueueFull`] or [`NetError::Closed`].
    pub fn stance(&self, stance: WireStance) -> Result<(), NetError> {
        self.send(NetCommand::Stance(stance))
    }

    /// Asks to change the player's appearance (`AppearanceSet`; the server must advertise
    /// `APPEARANCE_CHANGE`). The server answers with an [`Event::AppearanceChanged`] for
    /// the player carrying the result.
    ///
    /// # Errors
    ///
    /// [`NetError::InvalidCommand`] when the appearance cannot travel (invalid, or an
    /// object body with DID 0), [`NetError::SendQueueFull`] or [`NetError::Closed`].
    pub fn set_appearance(&self, appearance: &Appearance) -> Result<(), NetError> {
        let wire = convert::checked_wire_appearance(appearance)?;
        self.send(NetCommand::Appearance(Box::new(wire)))
    }
}

impl Session {
    /// The test-server backend of this session: the backend itself, or the server half of a
    /// scenario overlay (`OverlayBackend<RemoteBackend>`, `scenario-feed.md` §12.5). `None`
    /// for any other backend. Use this rather than `backend::<RemoteBackend>()`, which misses
    /// the overlay.
    #[must_use]
    pub fn remote_backend(&self) -> Option<&RemoteBackend> {
        if let Some(r) = self.backend::<RemoteBackend>() {
            return Some(r);
        }
        #[cfg(feature = "scenario")]
        if let Some(o) = self.backend::<crate::scenario::OverlayBackend<RemoteBackend>>() {
            return Some(o.server());
        }
        None
    }
}

/// Channels to the network thread once it runs.
#[derive(Debug)]
struct Running {
    commands: mpsc::Sender<NetCommand>,
    events: mpsc::Receiver<Inbound>,
    moves: watch::Sender<Option<MoveSlot>>,
    shutdown: watch::Sender<bool>,
    _task: tokio::task::JoinHandle<()>,
}

/// The test-server backend; see the [module documentation](self).
#[derive(Debug)]
pub struct RemoteBackend {
    config: Option<RemoteConfig>,
    base: Instant,
    running: Option<Running>,
    mapper: Mapper,
    shared: Arc<Mutex<RemoteStats>>,
    stats: RemoteStats,
    /// The interpolation delay in use (mirrors the mapper's configuration).
    delay: Duration,
    scratch: Vec<Event>,
}

impl RemoteBackend {
    /// A backend for `config`, without world data (no statics are streamed).
    ///
    /// # Errors
    ///
    /// [`NetError::InvalidConfig`] when the configuration breaks a protocol rule (see
    /// [`RemoteConfig::validate`]).
    pub fn new(config: RemoteConfig) -> Result<Self, NetError> {
        Self::build(config, None)
    }

    /// A backend that streams statics around the player from the client's own data, like
    /// the sandbox does.
    ///
    /// # Errors
    ///
    /// As [`new`](Self::new).
    pub fn with_world(
        config: RemoteConfig,
        source: impl WorldSource + 'static,
    ) -> Result<Self, NetError> {
        Self::build(config, Some(Box::new(source)))
    }

    /// As [`with_world`](Self::with_world), with an already boxed source.
    ///
    /// # Errors
    ///
    /// As [`new`](Self::new).
    pub fn with_boxed_world(
        config: RemoteConfig,
        source: Box<dyn WorldSource>,
    ) -> Result<Self, NetError> {
        Self::build(config, Some(source))
    }

    fn build(config: RemoteConfig, source: Option<Box<dyn WorldSource>>) -> Result<Self, NetError> {
        config.validate()?;
        let mapper = Mapper::new(config.statics.clone(), config.interp, source);
        Ok(Self {
            delay: config.interp.delay,
            config: Some(config),
            base: Instant::now(),
            running: None,
            mapper,
            shared: Arc::new(Mutex::new(RemoteStats::default())),
            stats: RemoteStats::default(),
            scratch: Vec::new(),
        })
    }

    fn start(&mut self) -> Result<(), NetError> {
        let Some(config) = self.config.take() else {
            return Err(NetError::Closed);
        };
        let credentials = Credentials {
            account: config.account.clone(),
            password: config.password.expose().to_owned(),
        };
        let client: ClientConfig = config.client.clone();
        let driver = Driver::new(client, credentials, config.register);
        let (cmd_tx, cmd_rx) = mpsc::channel(COMMAND_CHANNEL_CAPACITY);
        let (evt_tx, evt_rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
        let (move_tx, move_rx) = watch::channel(None);
        let (stop_tx, stop_rx) = watch::channel(false);
        let task = link::spawn(link::LinkParams {
            server: config.server.clone(),
            trust: config.trust.clone(),
            driver,
            base: self.base,
            commands: cmd_rx,
            events: evt_tx,
            moves: move_rx,
            shutdown: stop_rx,
            stats: Arc::clone(&self.shared),
        })?;
        self.running = Some(Running {
            commands: cmd_tx,
            events: evt_rx,
            moves: move_tx,
            shutdown: stop_tx,
            _task: task,
        });
        Ok(())
    }

    fn client_us(&self, now: Instant) -> u64 {
        u64::try_from(now.saturating_duration_since(self.base).as_micros()).unwrap_or(u64::MAX)
    }

    /// A handle for the commands beyond [`Command`] (emote, stance, appearance), once
    /// [`Command::Connect`] has been submitted.
    #[must_use]
    pub fn handle(&self) -> Option<RemoteHandle> {
        self.running.as_ref().map(|r| RemoteHandle {
            commands: r.commands.clone(),
        })
    }

    /// The statistics snapshot taken at the latest poll.
    #[must_use]
    pub fn stats(&self) -> &RemoteStats {
        &self.stats
    }

    /// Why the session failed, once it has (`None` for a clean, client-requested end).
    #[must_use]
    pub fn failure(&self) -> Option<&RemoteFailure> {
        self.mapper.failure.as_ref()
    }

    /// The server's `HelloAck` information, once greeted (available when
    /// [`Event::Connected`] is surfaced).
    #[must_use]
    pub fn server_info(&self) -> Option<&ServerInfo> {
        self.mapper.server.as_ref()
    }

    /// The server's capability bits from `HelloAck` (`tessera-protocol.md` §6.2.2), once
    /// greeted. A creation screen ghosts its New button unless
    /// [`ServerCaps::CHARACTER_CREATION`] is set (`character-screens.md` §7.1.1 step 1).
    #[must_use]
    pub fn server_caps(&self) -> Option<ServerCaps> {
        self.mapper.server.as_ref().map(|s| s.caps)
    }

    /// Whether the server advertised `CHARACTER_CREATION`: [`Command::Character`]
    /// `CreateCharacter` is accepted only then (`character-screens.md` §7.1.1).
    #[must_use]
    pub fn can_create_characters(&self) -> bool {
        self.server_caps()
            .is_some_and(|c| c.contains(ServerCaps::CHARACTER_CREATION))
    }

    /// Whether the server advertised `APPEARANCE_CHANGE` (`AppearanceSet` accepted).
    #[must_use]
    pub fn can_change_appearance(&self) -> bool {
        self.server_caps()
            .is_some_and(|c| c.contains(ServerCaps::APPEARANCE_CHANGE))
    }

    /// Whether the server takes named chat channels: negotiated minor ≥ 2 and
    /// `CHAT_CHANNELS` advertised (`tessera-protocol.md` §10.6). Only then are
    /// `JoinChannel`, `LeaveChannel` and `Chat` on `Named` sent (`scenario-feed.md` §9.8);
    /// otherwise they are refused with [`NetError::InvalidCommand`].
    #[must_use]
    pub fn has_chat_channels(&self) -> bool {
        self.mapper.chat_channels()
    }

    /// The negotiated minor protocol version, once greeted (1 carries looks and slots,
    /// 2 named chat channels; `tessera-protocol.md` §3.1).
    #[must_use]
    pub fn negotiated_minor(&self) -> Option<u16> {
        self.mapper.server.as_ref().map(|s| s.minor)
    }

    /// The name of the character creation sent and not yet answered.
    #[must_use]
    pub fn outstanding_creation(&self) -> Option<&str> {
        self.mapper.outstanding_create.as_deref()
    }

    /// The estimated server time (µs, §11.1) at `now`: the client clock plus the clock
    /// offset (§11.2). Before the first time sample it falls back to the newest server
    /// timestamp received (no latency correction). `None` before anything was received.
    #[must_use]
    pub fn server_time(&self, now: Instant) -> Option<u64> {
        let client = i128::from(self.client_us(now));
        match self.stats.clock_offset_us {
            Some(off) => u64::try_from(client + i128::from(off)).ok(),
            None => self.mapper.last_server_time,
        }
    }

    /// The interpolation delay in use: the configured one, raised to
    /// [`recommended_delay`](crate::interp::recommended_delay) of the server's rates once
    /// they are known (§8.6).
    #[must_use]
    pub fn interpolation_delay(&self) -> Duration {
        self.delay
    }

    /// The render time (server µs) at `now`: estimated server time − the interpolation
    /// delay (§8.6).
    #[must_use]
    pub fn render_time(&self, now: Instant) -> Option<u64> {
        self.server_time(now).map(|t| render_time(t, self.delay))
    }

    /// The interpolated state of a remote entity at `now` (§8.6). `None` for the player
    /// (simulated locally), statics, and unknown entities.
    #[must_use]
    pub fn sample(&self, id: EntityId, now: Instant) -> Option<Sample> {
        self.mapper.interp.sample(id, self.render_time(now)?)
    }

    /// The interpolation buffers of every remote entity.
    #[must_use]
    pub fn interpolation(&self) -> &crate::interp::InterpolationSet {
        &self.mapper.interp
    }

    /// The replicated properties of an entity or the player (`STANCE`, `EMOTE`, speeds).
    #[must_use]
    pub fn entity_state(&self, id: EntityId) -> Option<&EntityState> {
        self.mapper.state(id)
    }

    /// The server's entity ID for a local [`EntityId`] that is currently known: the player
    /// or a created remote entity (the same number unless it was at or above
    /// [`LOCAL_ID_BASE`]). `None` for statics and for IDs not created (or destroyed), like
    /// [`local_entity`](Self::local_entity).
    #[must_use]
    pub fn server_entity(&self, id: EntityId) -> Option<u64> {
        self.mapper.known_server_id(id)
    }

    /// The local [`EntityId`] of a server entity that is currently created (or the
    /// player).
    #[must_use]
    pub fn local_entity(&self, server: u64) -> Option<EntityId> {
        self.mapper.local_id(server)
    }

    /// Number of remote entities currently created (not the player, not statics).
    #[must_use]
    pub fn entity_count(&self) -> usize {
        self.mapper.entity_count()
    }

    /// The latest `WorldClock` (§6.3.9).
    #[must_use]
    pub fn world_clock(&self) -> Option<&WorldClock> {
        self.mapper.world_clock.as_ref()
    }

    /// The game clock *G* in seconds at `now` (§11.3), from the latest `WorldClock` and the
    /// estimated server time. The client turns it into a time of day with its own data's
    /// constants.
    #[must_use]
    pub fn game_clock(&self, now: Instant) -> Option<f64> {
        let c = self.mapper.world_clock.as_ref()?;
        Some(c.game_clock_at(self.server_time(now)?))
    }

    /// Statics streaming counters (as [`SandboxBackend::static_stats`](crate::sandbox::SandboxBackend::static_stats)).
    #[must_use]
    pub fn static_stats(&self) -> StaticStats {
        self.mapper.static_stats()
    }

    /// The world source the backend streams statics from, if any.
    #[must_use]
    pub fn world_source(&self) -> Option<&dyn WorldSource> {
        self.mapper.source()
    }

    fn drain(&mut self, out: &mut Vec<Event>) {
        let mut ended_channel = false;
        if let Some(r) = self.running.as_mut() {
            loop {
                match r.events.try_recv() {
                    Ok(item) => self.mapper.map(item, out),
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        ended_channel = true;
                        break;
                    }
                }
            }
        }
        if ended_channel && !self.mapper.ended {
            self.mapper.map(
                Inbound::Closed {
                    reason: crate::DisconnectReason::TransportFailed(
                        "the network thread stopped".into(),
                    ),
                    failure: Some(RemoteFailure::Transport("network thread stopped".into())),
                },
                out,
            );
        }
    }
}

impl Backend for RemoteBackend {
    fn name(&self) -> &str {
        "tessera-server"
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn submit(&mut self, _now: Instant, command: Command) -> Result<(), NetError> {
        match route(command, &mut self.mapper)? {
            Routed::Connect => {
                if self.running.is_some() {
                    return Err(NetError::InvalidCommand("already connected"));
                }
                self.start()
            }
            Routed::Net(cmd) => {
                let r = self.running.as_ref().ok_or(NetError::Closed)?;
                let creation = cmd.creation();
                r.commands.try_send(cmd).map_err(|e| match e {
                    mpsc::error::TrySendError::Full(_) => NetError::SendQueueFull,
                    mpsc::error::TrySendError::Closed(_) => NetError::Closed,
                })?;
                if creation.is_some() {
                    self.mapper.outstanding_create = creation;
                }
                Ok(())
            }
            Routed::Move(slot) => {
                if let Some(r) = self.running.as_ref() {
                    r.moves.send_replace(Some(slot));
                }
                Ok(())
            }
            Routed::Disconnect => {
                if let Some(r) = self.running.as_ref() {
                    r.shutdown.send_replace(true);
                }
                Ok(())
            }
            Routed::Local => Ok(()),
        }
    }

    fn poll(&mut self, _now: Instant, out: &mut Vec<Event>) {
        if let Ok(s) = self.shared.try_lock() {
            self.stats.clone_from(&s);
        }
        let mut events = std::mem::take(&mut self.scratch);
        // A local selection answer goes first, before anything mapped in this poll (§9.7).
        self.mapper.deliver_local(&mut events);
        self.drain(&mut events);
        self.delay = self.mapper.interp.config().delay;
        self.mapper.update_statics(&mut events);
        out.append(&mut events);
        self.scratch = events;
    }
}

impl Drop for RemoteBackend {
    fn drop(&mut self) {
        // The network thread sees the shutdown (or the dropped channels), says Disconnect,
        // and exits on its own; nothing here waits for it.
        if let Some(r) = self.running.as_ref() {
            r.shutdown.send_replace(true);
        }
    }
}

/// A [`Session`] over a [`RemoteBackend`] for `config`.
///
/// # Errors
///
/// As [`RemoteBackend::new`].
pub fn session(config: RemoteConfig) -> Result<Session, NetError> {
    Ok(Session::new(RemoteBackend::new(config)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gameplay_commands_and_new_channels_are_refused() {
        use crate::domain::{CombatCommand, Ref};
        let mut m = Mapper::new(
            crate::sandbox::world::StaticConfig::default(),
            crate::interp::InterpConfig::default(),
            None,
        );
        let refused = |r: Result<Routed, NetError>| matches!(r, Err(NetError::InvalidCommand(_)));
        assert!(refused(route(
            Command::Combat(CombatCommand::UseSkill {
                what: Ref::Literal("x".into()),
                target: None
            }),
            &mut m
        )));
        for channel in [
            crate::ChatChannel::Fellowship,
            crate::ChatChannel::Named {
                name: "Lore".into(),
            },
            crate::ChatChannel::System,
        ] {
            assert!(refused(route(
                Command::Chat {
                    channel,
                    text: "hi".into()
                },
                &mut m
            )));
        }
        assert!(matches!(
            route(
                Command::Chat {
                    channel: crate::ChatChannel::Say,
                    text: "hi".into()
                },
                &mut m
            ),
            Ok(Routed::Net(_))
        ));
    }

    /// `SelectTarget` never becomes network work (`scenario-feed.md` §9.7): without a player
    /// it is refused, and `UseSkill`/`CancelCast`/`SetQuickslot` stay refused.
    #[test]
    fn selection_is_local_and_needs_a_player() {
        use crate::domain::CombatCommand;
        let mut m = Mapper::new(
            crate::sandbox::world::StaticConfig::default(),
            crate::interp::InterpConfig::default(),
            None,
        );
        let select = Command::Combat(CombatCommand::SelectTarget { target: None });
        assert!(matches!(
            route(select.clone(), &mut m),
            Err(NetError::InvalidCommand(_))
        ));
        let mut out = Vec::new();
        m.deliver_local(&mut out);
        assert!(out.is_empty());
        for c in [
            CombatCommand::CancelCast,
            CombatCommand::SetQuickslot {
                slot: 1,
                content: None,
            },
        ] {
            assert!(matches!(
                route(Command::Combat(c), &mut m),
                Err(NetError::InvalidCommand(_))
            ));
        }
    }

    fn greeted(minor: u16, caps: ServerCaps) -> Mapper {
        let mut m = Mapper::new(
            crate::sandbox::world::StaticConfig::default(),
            crate::interp::InterpConfig::default(),
            None,
        );
        m.server = Some(ServerInfo {
            minor,
            caps,
            tick_rate: 30,
            snapshot_rate: 15,
            interest_radius: 2,
            name: String::new(),
            motd: String::new(),
        });
        m
    }

    /// `scenario-feed.md` §9.8 and §14.13 S5–S6: the channel commands go out only at
    /// minor ≥ 2 with `CHAT_CHANNELS`; every other Social command is refused at every minor.
    #[test]
    fn channel_commands_need_minor_2_and_the_capability() {
        let join = || {
            Command::Social(SocialCommand::JoinChannel {
                name: "Lore".into(),
            })
        };
        let leave = || {
            Command::Social(SocialCommand::LeaveChannel {
                name: "Lore".into(),
            })
        };
        let named = || Command::Chat {
            channel: crate::ChatChannel::Named {
                name: "Lore".into(),
            },
            text: "hi".into(),
        };
        let refused = |r: Result<Routed, NetError>| matches!(r, Err(NetError::InvalidCommand(_)));
        for (minor, caps) in [
            (1, ServerCaps::CHAT_CHANNELS),
            (2, ServerCaps::NONE),
            (1, ServerCaps::NONE),
        ] {
            let mut m = greeted(minor, caps);
            assert!(!m.chat_channels());
            for c in [join(), leave(), named()] {
                assert!(refused(route(c, &mut m)), "minor {minor}");
            }
        }
        let mut m = greeted(2, ServerCaps::CHAT_CHANNELS);
        assert!(m.chat_channels());
        assert!(matches!(
            route(join(), &mut m),
            Ok(Routed::Net(NetCommand::JoinChannel(n))) if n == "Lore"
        ));
        assert!(matches!(
            route(leave(), &mut m),
            Ok(Routed::Net(NetCommand::LeaveChannel(n))) if n == "Lore"
        ));
        assert!(matches!(
            route(named(), &mut m),
            Ok(Routed::Net(NetCommand::Chat(
                tessera_protocol::message::ChatChannel::Named { .. },
                _
            )))
        ));
        for c in [
            Command::Chat {
                channel: crate::ChatChannel::Fellowship,
                text: "hi".into(),
            },
            Command::Social(SocialCommand::ReadyPerformance {
                tune: "Reel".into(),
            }),
            Command::Social(SocialCommand::StartPerformance),
            Command::Social(SocialCommand::DisbandFellowship),
            Command::Social(SocialCommand::InviteToFellowship {
                name: "Brin".into(),
            }),
        ] {
            assert!(refused(route(c, &mut m)));
        }
    }

    /// A creation whose command never reaches the network thread is not outstanding
    /// (`character-screens.md` §7.1.1 step 2): a full or closed command queue leaves later
    /// creations and `EnterWorld` allowed.
    #[test]
    fn unsent_creation_is_not_outstanding() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let config =
            RemoteConfig::new("tessera://127.0.0.1:4000", "alice", Password::new("pw")).unwrap();
        let mut b = RemoteBackend::new(config).unwrap();
        b.mapper.server = Some(ServerInfo {
            minor: 1,
            caps: ServerCaps::CHARACTER_CREATION,
            tick_rate: 30,
            snapshot_rate: 15,
            interest_radius: 2,
            name: "t".into(),
            motd: String::new(),
        });
        let (cmd_tx, mut cmd_rx) = mpsc::channel(1);
        let (_evt_tx, evt_rx) = mpsc::channel(1);
        let (move_tx, _move_rx) = watch::channel(None);
        let (stop_tx, _stop_rx) = watch::channel(false);
        b.running = Some(Running {
            commands: cmd_tx,
            events: evt_rx,
            moves: move_tx,
            shutdown: stop_tx,
            _task: rt.spawn(async {}),
        });
        let create = |name: &str| {
            Command::Character(CharacterCommand::CreateCharacter {
                name: name.into(),
                appearance: Some(Box::new(crate::Appearance::new(crate::Body::Character {
                    species: 23,
                    sex: 1,
                }))),
                class: None,
                nationality: None,
            })
        };
        let now = Instant::now();
        // Fill the queue, then the creation does not fit.
        b.submit(
            now,
            Command::Chat {
                channel: crate::ChatChannel::Say,
                text: "hi".into(),
            },
        )
        .unwrap();
        assert!(matches!(
            b.submit(now, create("Cora")),
            Err(NetError::SendQueueFull)
        ));
        assert_eq!(b.outstanding_creation(), None);
        // A closed queue: also nothing recorded.
        drop(cmd_rx.try_recv());
        drop(cmd_rx);
        assert!(matches!(
            b.submit(now, create("Cora")),
            Err(NetError::Closed)
        ));
        assert_eq!(b.outstanding_creation(), None);
        // A queued one is outstanding.
        let (cmd_tx, _cmd_rx) = mpsc::channel(4);
        b.running.as_mut().unwrap().commands = cmd_tx;
        b.submit(now, create("Cora")).unwrap();
        assert_eq!(b.outstanding_creation(), Some("Cora"));
        assert!(matches!(
            b.submit(
                now,
                Command::EnterWorld {
                    character: "Aldith".into()
                }
            ),
            Err(NetError::InvalidCommand(_))
        ));
    }

    #[test]
    fn backend_is_send() {
        fn is_send<T: Send>() {}
        is_send::<RemoteBackend>();
        is_send::<RemoteHandle>();
    }

    #[test]
    fn motion_from_velocity() {
        let m = derive_motion(Vec3::new(6.0, 0.0, 0.0), 0.0, 2.0, 6.0);
        assert_eq!(m.gait, Gait::Running);
        assert!(!m.backward);
        let m = derive_motion(Vec3::new(-2.0, 0.0, 0.0), 0.0, 2.0, 6.0);
        assert_eq!(m.gait, Gait::Walking);
        assert!(m.backward);
        assert_eq!(derive_motion(Vec3::ZERO, 1.0, 2.0, 6.0), Motion::STANDING);
    }

    #[test]
    fn config_is_validated() {
        let ok = RemoteConfig::new("tessera://127.0.0.1:4000", "alice", Password::new("pw"));
        assert!(ok.is_ok());
        assert!(RemoteConfig::new("http://x:1", "a", Password::new("pw")).is_err());
        let bad =
            RemoteConfig::new("tessera://127.0.0.1:4000", "bad name", Password::new("pw")).unwrap();
        assert!(RemoteBackend::new(bad).is_err());
        let bad =
            RemoteConfig::new("tessera://127.0.0.1:4000", "alice", Password::new("")).unwrap();
        assert!(RemoteBackend::new(bad).is_err());
        let cfg = RemoteConfig::new("tessera://127.0.0.1:4000", "alice", Password::new("secret"))
            .unwrap();
        assert!(!format!("{cfg:?}").contains("secret"));
    }
}
