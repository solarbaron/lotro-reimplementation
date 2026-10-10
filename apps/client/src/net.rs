// SPDX-License-Identifier: GPL-3.0-or-later
//! World session glue: drives a [`Session`] once per frame, logs its events and keeps a
//! tiny entity table for the debug view. With [`World::capture_entity_events`] it also
//! queues the entity events (created, moved, appearance, despawned, cleared) for the world
//! walker's characters, bounded by [`MAX_QUEUED_ENTITY_EVENTS`] between two drains (see
//! there), and a drain hands out at most the number of events the caller asks for.
//!
//! The backend is the in-process sandbox, or with `--server` a Tessera test server
//! ([`World::remote`], [`World::for_walker`]; `tessera-net`'s `remote::RemoteBackend`,
//! `docs/specs/tessera-protocol.md`). With a remote backend, other entities are drawn from
//! snapshot interpolation ([`World::sample_entity`], §8.6), [`World::net_status`] has the
//! connection state, round-trip time and clock offset for the HUD, and the walker's chat
//! overlay reads [`World::drain_hud_chat`].

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use tessera_net::interp::Sample;
use tessera_net::remote::{
    ClientFailure, Fingerprint, FirstUse, Password, RemoteBackend, RemoteConfig, RemoteFailure,
    Trust,
};
use tessera_net::sandbox::world::WorldSource;
use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::{
    ChatChannel, Command, Destination, EntityId, EntityKind, Event, Location, LocationCause,
    NetError, Session, SessionState, Transform, Vec3,
};

/// How long [`World::shutdown`] waits for the backend to confirm the disconnect.
pub const DISCONNECT_GRACE: Duration = Duration::from_millis(500);
/// Bound on the entity events queued for the walker between two
/// [`World::drain_entity_events`]. Once the queue holds this many events, every queued move
/// is dropped and further moves are dropped on arrival (the next move of an entity
/// replaces them). Creations, appearance changes and removals are never dropped, so the
/// walker's entity set stays exact; instead an appearance change replaces the one still
/// queued for the same entity, and while this many of them are queued [`World::update`]
/// stops polling the backend. Nothing is read from the session until the walker drains,
/// so a flood backs up into the backend (for a remote session its bounded event channel
/// and then QUIC flow control) instead of growing this queue: it holds at most this many
/// events plus what one poll delivers.
pub const MAX_QUEUED_ENTITY_EVENTS: usize = 1 << 18;
/// Most chat lines one message of the day shows (`tessera-protocol.md` §6.2.2).
pub const MAX_MOTD_LINES: usize = 8;
/// Chat lines kept for the walker's chat overlay ([`World::drain_hud_chat`]); older lines
/// are dropped when nobody drains them (no walker yet).
pub const HUD_CHAT_LINES: usize = 64;
/// File name of the known-servers file in Tessera's settings directory
/// (`tessera-protocol.md` §4.5).
pub const KNOWN_SERVERS_FILE: &str = "known_servers";

/// How to reach and log in to a Tessera test server (`--server`, `--account`, ...).
/// `Debug` never shows the password.
#[derive(Debug, Clone)]
pub struct RemoteSettings {
    /// Connect string `tessera://HOST:PORT[?pin=sha256:...]`.
    pub server: String,
    /// The fingerprint to trust (`--server-fingerprint`); it must agree with a pin in the
    /// connect string, which is authoritative (`tessera-protocol.md` §4.5).
    pub fingerprint: Option<Fingerprint>,
    /// The known-servers file consulted without a pin (`None`: none).
    pub known_servers: Option<PathBuf>,
    /// Accept an unknown server key on first use and record it (an explicit user choice).
    pub accept_new_key: bool,
    /// Account name.
    pub account: String,
    /// Password (never logged).
    pub password: Password,
    /// Character to enter, matched by name key (NFC, then lower case; `test-server.md`
    /// §6.4); the account's first character when `None`. A name the account does not have
    /// is not entered: the session is ended with a system line listing the characters.
    pub character: Option<String>,
}

impl RemoteSettings {
    /// The backend configuration (default interpolation and statics parameters).
    ///
    /// # Errors
    ///
    /// [`NetError::InvalidConfig`] when the connect string does not parse, or when it pins
    /// a fingerprint other than [`RemoteSettings::fingerprint`].
    pub fn config(&self) -> Result<RemoteConfig, NetError> {
        let mut config = RemoteConfig::new(&self.server, &self.account, self.password.clone())?
            .with_trust(Trust {
                known_servers: self.known_servers.clone(),
                first_use: if self.accept_new_key {
                    FirstUse::Accept
                } else {
                    FirstUse::Refuse
                },
            });
        if let Some(fp) = self.fingerprint {
            if config.server.pin.is_some_and(|pin| pin != fp) {
                return Err(NetError::InvalidConfig(
                    "--server-fingerprint differs from the connect string's pin",
                ));
            }
            config = config.pinned(fp);
        }
        config.client.client_name = format!("tessera-client {}", env!("CARGO_PKG_VERSION"));
        Ok(config)
    }
}

/// The default known-servers file: next to Tessera's settings file
/// (`tessera_settings::default_path`), `None` when no settings directory is known.
#[must_use]
pub fn default_known_servers() -> Option<PathBuf> {
    tessera_settings::default_path()
        .ok()
        .map(|p| p.with_file_name(KNOWN_SERVERS_FILE))
}

/// What the HUD shows about the world session.
#[derive(Debug, Clone, PartialEq)]
pub struct NetStatus {
    /// Backend name (`sandbox`, `tessera-server`).
    pub backend: String,
    /// Session state.
    pub state: SessionState,
    /// The remote protocol state machine's state (`None` for the sandbox).
    pub link: Option<&'static str>,
    /// Latest round-trip time (application ping, else the QUIC path estimate).
    pub rtt: Option<Duration>,
    /// Clock offset server − client in µs (§11.2), once estimated.
    pub clock_offset_us: Option<i64>,
    /// Interpolation delay in use (remote only).
    pub delay: Option<Duration>,
    /// Remote entities currently created (remote only).
    pub entities: usize,
}

/// A visible entity, as last reported by the backend.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityView {
    /// Classification.
    pub kind: EntityKind,
    /// Display name.
    pub name: String,
    /// Last transform (metres, +Z up).
    pub transform: Transform,
}

/// A chat line for the plugin host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatLine {
    /// Sender, empty for system messages.
    pub sender: String,
    /// Text.
    pub text: String,
    /// Channel it arrived on.
    pub channel: ChatChannel,
    /// The line's chat type for the game UI's chat window (`chat-window.md` §2): 5 `Say`,
    /// 6 `Tell`, 8 `Log` for the session's system lines (`chat-window.md` §10 item 2), 4
    /// `Standard` for the other channels.
    pub chat_type: u32,
}

impl ChatLine {
    /// A line from the session's `channel`.
    #[must_use]
    pub fn from_channel(channel: &ChatChannel, sender: Option<&str>, text: &str) -> Self {
        let chat_type = match channel {
            ChatChannel::Say => 5,
            ChatChannel::Tell { .. } => 6,
            ChatChannel::System => 8,
            _ => 4,
        };
        Self {
            sender: sender.unwrap_or_default().to_owned(),
            text: text.to_owned(),
            channel: channel.clone(),
            chat_type: if sender.is_none_or(str::is_empty) && chat_type != 6 {
                8
            } else {
                chat_type
            },
        }
    }

    /// A system line of the session (no sender, [`ChatChannel::System`], type 8 `Log`).
    #[must_use]
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            sender: String::new(),
            text: text.into(),
            channel: ChatChannel::System,
            chat_type: 8,
        }
    }
}

/// The player's precise position, from the latest location report.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerLocation {
    /// Where (region-qualified, f64).
    pub location: Location,
    /// Facing in radians (0 = east, counter-clockwise).
    pub heading: f32,
    /// Why it was reported.
    pub cause: LocationCause,
    /// Increases with every report for the lifetime of the [`World`] (also across a
    /// disconnect), so callers can tell a new report from an old one.
    pub seq: u64,
}

/// The HUD's scenario line for `status` (`scenario-feed.md` §12.4): the state and the
/// virtual time; `lag_ms` (how far the feed is behind the frame time) is shown above a
/// quarter second; `closed` (the session has ended) shows a running scenario as stopped.
#[must_use]
pub fn scenario_line_text(
    status: &tessera_net::scenario::ScenarioStatus,
    lag_ms: u64,
    closed: bool,
) -> String {
    use tessera_net::scenario::{FinishReason, ScenarioStatus};
    let secs = |ms: u64| ms as f64 / 1e3;
    match status {
        ScenarioStatus::Running { time_ms } if closed => {
            format!("Scenario stopped (disconnected) at {:.3} s", secs(*time_ms))
        }
        ScenarioStatus::Running { time_ms } => {
            let mut t = format!("Scenario running {:.3} s", secs(*time_ms));
            if lag_ms > 250 {
                t.push_str(&format!(" (lag {:.1} s)", secs(lag_ms)));
            }
            t
        }
        ScenarioStatus::Finished { time_ms, reason } => {
            let why = match reason {
                FinishReason::Horizon => "horizon",
                _ => "complete",
            };
            format!("Scenario finished ({why}) at {:.3} s", secs(*time_ms))
        }
        ScenarioStatus::Failed { time_ms, error } => {
            format!("Scenario failed at {:.3} s: {error}", secs(*time_ms))
        }
        _ => "Scenario".to_owned(),
    }
}

impl crate::walker::WalkerSession for World {
    fn player_location(&self) -> Option<PlayerLocation> {
        World::player_location(self)
    }

    fn send_move(&mut self, now: Instant, location: Location, heading: f32, velocity: Vec3) {
        World::send_move(self, now, location, heading, velocity);
    }

    fn player_id(&self) -> Option<EntityId> {
        self.player
    }

    fn drain_entity_events(&mut self, out: &mut Vec<Event>, max: usize) {
        World::drain_entity_events(self, out, max);
    }

    fn sample_entity(&self, id: EntityId, now: Instant) -> Option<Sample> {
        World::sample_entity(self, id, now)
    }

    fn net_status(&self) -> Option<NetStatus> {
        Some(World::net_status(self))
    }

    fn drain_chat(&mut self, out: &mut Vec<ChatLine>) {
        World::drain_hud_chat(self, out);
    }

    fn send_chat(&mut self, now: Instant, channel: &ChatChannel, text: &str) -> bool {
        match World::try_chat(self, now, channel.clone(), text) {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!("chat not sent: {e}");
                false
            }
        }
    }

    fn scenario_line(&self, now: Instant) -> Option<String> {
        World::scenario_line(self, now)
    }
}

/// The client's view of the world source.
#[derive(Debug)]
pub struct World {
    session: Session,
    events: Vec<Event>,
    entities: BTreeMap<EntityId, EntityView>,
    player: Option<EntityId>,
    location: Option<PlayerLocation>,
    /// Sequence number of the last location report; never reset (not even on a
    /// disconnect), so a report after a reconnect is always new to callers.
    location_seq: u64,
    chat: Vec<ChatLine>,
    events_seen: u64,
    moves_seen: u64,
    /// Entities placed by the world data (removed by `StaticsCleared`).
    placed: std::collections::BTreeSet<EntityId>,
    /// Entity events for the walker (`None`: not captured).
    entity_events: Option<VecDeque<Event>>,
    /// Moves in `entity_events`.
    queued_moves: usize,
    /// [`MAX_QUEUED_ENTITY_EVENTS`] (smaller in tests).
    queue_cap: usize,
    /// Moves dropped because the queue was full.
    dropped_moves: u64,
    /// Recent chat lines for the walker's overlay (at most [`HUD_CHAT_LINES`]).
    hud_chat: VecDeque<ChatLine>,
    /// Character to enter (remote sessions; the first one otherwise).
    character: Option<String>,
    /// When the player entered the world (the scenario clock's origin, for the HUD's lag).
    entered_at: Option<Instant>,
    /// The server key was pinned (connect string or `--server-fingerprint`), so the
    /// known-servers file played no part (`tessera-protocol.md` §4.5).
    pinned: bool,
    /// Sequence number of the front of `entity_events` (events are numbered as queued).
    head_seq: u64,
    /// The queued `AppearanceChanged` of an entity (its sequence number), replaced in
    /// place by a newer one.
    queued_appearance: HashMap<EntityId, u64>,
    /// Appearance changes replaced by a newer one before the walker saw them.
    coalesced_appearances: u64,
    /// Polling is paused until the walker drains (logged once per pause).
    paused: bool,
    /// The message of the day was shown.
    motd_shown: bool,
    /// `--scenario-log` of a windowed scenario run (`scenario-feed.md` §12.4).
    scenario_log: Option<crate::scenario::live_log::LiveLog>,
    /// `--scenario-input` of a windowed scenario run (`scenario-feed.md` §7.7, §12.4): the
    /// input script's player.
    scenario_input: Option<tessera_net::scenario::InputPlayer>,
    /// After the input script's `at pre` lines: the character to enter, and whether the
    /// poll that delivers their answers has happened (the world is entered after it).
    pending_enter: Option<(Option<String>, bool)>,
}

impl World {
    /// A session with the in-process sandbox backend, already asked to connect.
    pub fn sandbox(now: Instant) -> Result<Self, NetError> {
        Self::start(now, SandboxBackend::new(SandboxConfig::default())?)
    }

    /// A sandbox session with `config` and no world data, already asked to connect.
    pub fn sandbox_with(now: Instant, config: SandboxConfig) -> Result<Self, NetError> {
        Self::start(now, SandboxBackend::new(config)?)
    }

    /// A sandbox session that resolves places, heights and descriptions through `source`
    /// (the world walker), already asked to connect.
    pub fn sandbox_with_world(
        now: Instant,
        config: SandboxConfig,
        source: impl WorldSource + 'static,
    ) -> Result<Self, NetError> {
        Self::start(now, SandboxBackend::with_world(config, source)?)
    }

    /// A session with a Tessera test server, already asked to connect. With `source` the
    /// backend streams statics around the player from the client's own data, as the
    /// sandbox does.
    ///
    /// # Errors
    ///
    /// [`NetError::InvalidConfig`] for settings the protocol refuses (connect string,
    /// account name, empty password).
    pub fn remote(
        now: Instant,
        settings: &RemoteSettings,
        source: Option<Box<dyn WorldSource>>,
        statics: Option<tessera_net::sandbox::world::StaticConfig>,
    ) -> Result<Self, NetError> {
        let mut config = settings.config()?;
        if let Some(s) = statics {
            config.statics = s;
        }
        let pinned = config.server.pin.is_some();
        tracing::info!(
            server = %config.server,
            account = %settings.account,
            pinned = config.server.pin.is_some(),
            "connecting to a Tessera test server"
        );
        let backend = match source {
            Some(s) => RemoteBackend::with_boxed_world(config, s)?,
            None => RemoteBackend::new(config)?,
        };
        let mut world = Self::from_session(now, Session::new(backend))?;
        world.character.clone_from(&settings.character);
        world.pinned = pinned;
        Ok(world)
    }

    /// The walker's session: the test server of `remote` when given (statics streamed from
    /// `source` with `config`'s static settings), else the sandbox with `config`.
    ///
    /// # Errors
    ///
    /// As [`World::remote`] or [`World::sandbox_with_world`].
    pub fn for_walker(
        now: Instant,
        remote: Option<&RemoteSettings>,
        config: SandboxConfig,
        source: impl WorldSource + 'static,
    ) -> Result<Self, NetError> {
        match remote {
            Some(r) => Self::remote(now, r, Some(Box::new(source)), Some(config.statics)),
            None => Self::sandbox_with_world(now, config, source),
        }
    }

    /// A session without world data: the test server of `remote` when given, else the
    /// default sandbox.
    ///
    /// # Errors
    ///
    /// As [`World::remote`] or [`World::sandbox`].
    pub fn plain(now: Instant, remote: Option<&RemoteSettings>) -> Result<Self, NetError> {
        match remote {
            Some(r) => Self::remote(now, r, None, None),
            None => Self::sandbox(now),
        }
    }

    fn start(now: Instant, backend: SandboxBackend) -> Result<Self, NetError> {
        Self::from_session(now, Session::new(backend))
    }

    fn from_session(now: Instant, mut session: Session) -> Result<Self, NetError> {
        tracing::info!(backend = session.backend_name(), "world session created");
        session.send(now, Command::Connect)?;
        Ok(Self {
            session,
            events: Vec::new(),
            entities: BTreeMap::new(),
            player: None,
            location: None,
            location_seq: 0,
            chat: Vec::new(),
            events_seen: 0,
            moves_seen: 0,
            placed: std::collections::BTreeSet::new(),
            entity_events: None,
            queued_moves: 0,
            queue_cap: MAX_QUEUED_ENTITY_EVENTS,
            dropped_moves: 0,
            hud_chat: VecDeque::new(),
            character: None,
            entered_at: None,
            pinned: false,
            head_seq: 0,
            queued_appearance: HashMap::new(),
            coalesced_appearances: 0,
            paused: false,
            motd_shown: false,
            scenario_log: None,
            scenario_input: None,
            pending_enter: None,
        })
    }

    /// Sets the character to enter when the session connects (a scenario's `enter`
    /// character, `scenario-feed.md` §4.3); `None` enters the first one offered. A name the
    /// list does not hold is not entered: the session ends with a system line listing the
    /// characters.
    pub fn set_character(&mut self, name: Option<String>) {
        self.character = name;
    }

    /// Writes the scenario's canonical log from now on (`--scenario-log` of a windowed run,
    /// `scenario-feed.md` §12.4): every command this session sends and every event it
    /// receives, with wall-clock times, until the end line. Call right after creating the
    /// session: the log's first command line is the `Connect` the constructor sent.
    pub fn set_scenario_log(&mut self, mut log: crate::scenario::live_log::LiveLog, now: Instant) {
        log.command(now, &Command::Connect);
        self.scenario_log = Some(log);
    }

    /// Plays the scenario's input script (`--scenario-input` of a windowed run,
    /// `scenario-feed.md` §12.4) from now on: its `at pre` lines after `Connected` (the
    /// world is entered one poll later, so a character they create can be entered), and
    /// each timed line before the poll that lets the feed process its tick. Call right
    /// after creating the session.
    pub fn set_scenario_input(&mut self, player: tessera_net::scenario::InputPlayer) {
        self.scenario_input = Some(player);
    }

    /// Sends an input line's command, or counts the line as skipped (§7.7).
    fn send_input(
        &mut self,
        now: Instant,
        player: &mut tessera_net::scenario::InputPlayer,
        line: tessera_net::scenario::InputLine,
    ) {
        match line.command {
            Some(c) => {
                if let Err(e) = self.send(now, c) {
                    tracing::debug!("scenario input line {} refused: {e}", line.line);
                    player.skip(line.line, "refused by the session");
                }
            }
            None => player.skip(line.line, "target does not exist"),
        }
    }

    /// Sends the timed input lines due by the tick the feed processes next.
    fn play_due_input(&mut self, now: Instant) {
        use tessera_net::scenario::ScenarioStatus;
        if self.player.is_none() {
            return;
        }
        let Some(ScenarioStatus::Running { time_ms }) = self.scenario_status() else {
            return;
        };
        let Some(mut player) = self.scenario_input.take() else {
            return;
        };
        let next_tick = (time_ms / player.tick_ms()).saturating_add(1);
        while let Some(line) = player.next_due(next_tick) {
            self.send_input(now, &mut player, line);
        }
        self.scenario_input = Some(player);
    }

    /// Sends the input script's `at pre` lines; returns whether there were any.
    fn play_pre_input(&mut self, now: Instant) -> bool {
        let Some(mut player) = self.scenario_input.take() else {
            return false;
        };
        let any = player.has_pre();
        while let Some(line) = player.next_pre() {
            self.send_input(now, &mut player, line);
        }
        self.scenario_input = Some(player);
        any
    }

    /// Sends `command`, writing it to the scenario log when the session accepts it.
    fn send(&mut self, now: Instant, command: Command) -> Result<(), NetError> {
        match &mut self.scenario_log {
            Some(log) if !log.is_closed() => {
                let logged = command.clone();
                self.session.send(now, command)?;
                log.command(now, &logged);
                Ok(())
            }
            _ => self.session.send(now, command),
        }
    }

    /// The scenario's status when the sandbox plays one (`scenario-feed.md` §3.4).
    #[must_use]
    pub fn scenario_status(&self) -> Option<tessera_net::scenario::ScenarioStatus> {
        self.session
            .backend::<SandboxBackend>()
            .and_then(SandboxBackend::scenario_status)
    }

    /// The HUD's scenario line (`scenario-feed.md` §12.4): the state, the virtual time and,
    /// when the feed lags more than a quarter second behind the frame time, the lag; a
    /// running scenario whose session has ended shows as stopped.
    #[must_use]
    pub fn scenario_line(&self, now: Instant) -> Option<String> {
        use tessera_net::scenario::ScenarioStatus;
        let status = self.scenario_status()?;
        let lag_ms = match status {
            ScenarioStatus::Running { time_ms } => self
                .entered_at
                .map(|at| now.saturating_duration_since(at).as_millis())
                .and_then(|wall| u64::try_from(wall).ok())
                .map_or(0, |wall| wall.saturating_sub(time_ms)),
            _ => 0,
        };
        Some(scenario_line_text(
            &status,
            lag_ms,
            self.state() == SessionState::Closed,
        ))
    }

    /// The test-server backend, when this session uses one.
    #[must_use]
    pub fn remote_backend(&self) -> Option<&RemoteBackend> {
        self.session.backend::<RemoteBackend>()
    }

    /// The interpolated state of a remote entity at `now` (`tessera-protocol.md` §8.6:
    /// render time = estimated server time − the interpolation delay). `None` with the
    /// sandbox (its characters are extrapolated by the walker), for the player and for
    /// unknown entities.
    #[must_use]
    pub fn sample_entity(&self, id: EntityId, now: Instant) -> Option<Sample> {
        self.remote_backend()?.sample(id, now)
    }

    /// Connection state, round-trip time and clock offset for the HUD.
    #[must_use]
    pub fn net_status(&self) -> NetStatus {
        let mut s = NetStatus {
            backend: self.session.backend_name().to_owned(),
            state: self.state(),
            link: None,
            rtt: None,
            clock_offset_us: None,
            delay: None,
            entities: 0,
        };
        if let Some(r) = self.remote_backend() {
            let st = r.stats();
            s.link = Some(st.state);
            s.rtt = st.ping_rtt.or(st.path_rtt);
            s.clock_offset_us = st.clock_offset_us;
            s.delay = Some(r.interpolation_delay());
            s.entities = r.entity_count();
        }
        s
    }

    /// Moves the chat lines received since the last call into `out` (oldest first; at
    /// most [`HUD_CHAT_LINES`] are kept between calls).
    pub fn drain_hud_chat(&mut self, out: &mut Vec<ChatLine>) {
        out.extend(self.hud_chat.drain(..));
    }

    fn push_chat(&mut self, line: ChatLine) {
        if self.hud_chat.len() >= HUD_CHAT_LINES {
            self.hud_chat.pop_front();
        }
        self.hud_chat.push_back(line.clone());
        self.chat.push(line);
    }

    /// Starts (or stops) queueing entity events for [`World::drain_entity_events`].
    /// Stopping drops what is queued.
    pub fn capture_entity_events(&mut self, on: bool) {
        self.entity_events = on.then(VecDeque::new);
        self.queued_moves = 0;
        self.head_seq = 0;
        self.queued_appearance.clear();
    }

    /// Moves at most `max` of the queued entity events into `out` (appended, oldest
    /// first); the rest stay queued, in order, for the next call.
    pub fn drain_entity_events(&mut self, out: &mut Vec<Event>, max: usize) {
        let Some(q) = &mut self.entity_events else {
            return;
        };
        let n = q.len().min(max);
        for (seq, ev) in (self.head_seq..).zip(q.drain(..n)) {
            match &ev {
                Event::EntityMoved { .. } => {
                    self.queued_moves = self.queued_moves.saturating_sub(1);
                }
                Event::AppearanceChanged { id, .. }
                    if self.queued_appearance.get(id) == Some(&seq) =>
                {
                    self.queued_appearance.remove(id);
                }
                _ => {}
            }
            out.push(ev);
        }
        self.head_seq += n as u64;
    }

    /// Whether the queue for the walker is full of events that are never dropped, so
    /// [`World::update`] leaves the backend's events where they are.
    fn queue_saturated(&self) -> bool {
        self.entity_events
            .as_ref()
            .is_some_and(|q| q.len().saturating_sub(self.queued_moves) >= self.queue_cap)
    }

    /// Rebuilds [`World::queued_appearance`] after the queue was compacted.
    fn reindex_appearances(&mut self) {
        self.queued_appearance.clear();
        let Some(q) = &self.entity_events else {
            return;
        };
        for (seq, ev) in (self.head_seq..).zip(q.iter()) {
            match ev {
                Event::AppearanceChanged { id, .. } => {
                    self.queued_appearance.insert(*id, seq);
                }
                Event::EntityCreated { id, .. } | Event::EntityDespawned { id } => {
                    self.queued_appearance.remove(id);
                }
                Event::StaticsCleared | Event::Disconnected { .. } => {
                    self.queued_appearance.clear();
                }
                _ => {}
            }
        }
    }

    /// Entity events waiting for [`World::drain_entity_events`].
    #[must_use]
    pub fn queued_entity_events(&self) -> usize {
        self.entity_events.as_ref().map_or(0, VecDeque::len)
    }

    fn queue_entity_event(&mut self, ev: &Event) {
        let Some(q) = &mut self.entity_events else {
            return;
        };
        // An appearance change replaces the one still queued for the same entity, in place
        // (the walker only needs the newest).
        match ev {
            Event::AppearanceChanged { id, .. } => {
                if let Some(&seq) = self.queued_appearance.get(id)
                    && let Some(slot) = seq
                        .checked_sub(self.head_seq)
                        .and_then(|i| usize::try_from(i).ok())
                        .and_then(|i| q.get_mut(i))
                    && matches!(slot, Event::AppearanceChanged { id: queued, .. } if queued == id)
                {
                    *slot = ev.clone();
                    self.coalesced_appearances += 1;
                    return;
                }
            }
            Event::EntityCreated { id, .. } | Event::EntityDespawned { id } => {
                self.queued_appearance.remove(id);
            }
            Event::StaticsCleared | Event::Disconnected { .. } => self.queued_appearance.clear(),
            _ => {}
        }
        let is_move = matches!(ev, Event::EntityMoved { .. });
        let mut compacted = false;
        if q.len() >= self.queue_cap {
            if self.queued_moves > 0 {
                // Drop every queued move at once (amortised O(1) per event; the
                // entities' next moves replace them).
                let before = q.len();
                q.retain(|e| !matches!(e, Event::EntityMoved { .. }));
                self.dropped_moves += (before - q.len()) as u64;
                self.queued_moves = 0;
                compacted = true;
                tracing::warn!(
                    dropped = self.dropped_moves,
                    "entity event queue full; queued moves dropped"
                );
            }
            if is_move && q.len() >= self.queue_cap {
                self.dropped_moves += 1;
                if compacted {
                    self.reindex_appearances();
                }
                return;
            }
        }
        if is_move {
            self.queued_moves += 1;
        }
        let seq = self.head_seq + q.len() as u64;
        q.push_back(ev.clone());
        if compacted {
            self.reindex_appearances();
        } else if let Event::AppearanceChanged { id, .. } = ev {
            self.queued_appearance.insert(*id, seq);
        }
    }

    /// The player's latest precise position.
    #[must_use]
    pub fn player_location(&self) -> Option<PlayerLocation> {
        self.location
    }

    /// Reports the player's movement (ignored before the player is in the world).
    pub fn send_move(&mut self, now: Instant, location: Location, heading: f32, velocity: Vec3) {
        if self.player.is_none() || self.state() != SessionState::InWorld {
            return;
        }
        if let Err(e) = self.send(
            now,
            Command::Move {
                location,
                heading,
                velocity,
            },
        ) {
            tracing::debug!("move not sent: {e}");
        }
    }

    /// Asks to be moved to `destination`.
    pub fn teleport(&mut self, now: Instant, destination: Destination) {
        if let Err(e) = self.send(now, Command::Teleport { destination }) {
            tracing::warn!("teleport not sent: {e}");
        }
    }

    /// Asks where the player is (answered with a location report and a description).
    pub fn query_location(&mut self, now: Instant) {
        if let Err(e) = self.send(now, Command::QueryLocation) {
            tracing::debug!("location query not sent: {e}");
        }
    }

    /// Session state.
    #[must_use]
    pub fn state(&self) -> SessionState {
        self.session.state()
    }

    /// Known entities.
    #[must_use]
    pub fn entities(&self) -> &BTreeMap<EntityId, EntityView> {
        &self.entities
    }

    /// The player entity, once in the world.
    #[must_use]
    pub fn player(&self) -> Option<EntityId> {
        self.player
    }

    /// Total events processed.
    #[must_use]
    pub fn events_seen(&self) -> u64 {
        self.events_seen
    }

    /// Chat lines received since the last call.
    pub fn take_chat(&mut self) -> Vec<ChatLine> {
        std::mem::take(&mut self.chat)
    }

    /// Say something on the local channel (a failure is logged; see [`World::try_say`]).
    pub fn say(&mut self, now: Instant, text: &str) {
        if let Err(e) = self.try_say(now, text) {
            tracing::warn!("chat not sent: {e}");
        }
    }

    /// Say something on the local channel.
    ///
    /// # Errors
    ///
    /// As `Session::send`: [`NetError::InvalidState`] when not in the world (connecting,
    /// failed, disconnected), [`NetError::InvalidCommand`] for text the protocol refuses.
    pub fn try_say(&mut self, now: Instant, text: &str) -> Result<(), NetError> {
        self.try_chat(now, ChatChannel::Say, text)
    }

    /// Sends `text` on `channel` (decision 0020: the dispatcher's channel commands).
    ///
    /// # Errors
    ///
    /// As [`World::try_say`]; also [`NetError::InvalidCommand`] for the system channel or
    /// a tell recipient the protocol refuses.
    pub fn try_chat(
        &mut self,
        now: Instant,
        channel: ChatChannel,
        text: &str,
    ) -> Result<(), NetError> {
        self.send(
            now,
            Command::Chat {
                channel,
                text: text.to_owned(),
            },
        )
    }

    /// Poll the backend and apply its events. Call once per frame. While the walker's
    /// queue is full of events that are never dropped ([`MAX_QUEUED_ENTITY_EVENTS`]),
    /// nothing is polled until it drains.
    pub fn update(&mut self, now: Instant) {
        if self.queue_saturated() {
            if !self.paused {
                tracing::warn!(
                    queued = self.queued_entity_events(),
                    "entity event queue full; not reading the session until the walker drains"
                );
            }
            self.paused = true;
            return;
        }
        self.paused = false;
        self.play_due_input(now);
        self.session.poll_into(now, &mut self.events);
        let events = std::mem::take(&mut self.events);
        let mut disconnected = false;
        for ev in &events {
            if let Some(log) = &mut self.scenario_log {
                log.event(now, ev);
            }
            if let Some(input) = &mut self.scenario_input {
                input.observe(ev);
            }
            disconnected |= matches!(ev, Event::Disconnected { .. });
            self.apply(now, ev);
        }
        self.events = events;
        self.events.clear();
        self.enter_after_pre_input(now, disconnected);
        if self.scenario_log.as_ref().is_some_and(|l| !l.is_closed()) {
            let status = self.scenario_status();
            let counters = self
                .session
                .backend::<SandboxBackend>()
                .and_then(SandboxBackend::scenario_counters);
            let closed = disconnected || self.state() == SessionState::Closed;
            let input_skipped = self
                .scenario_input
                .as_ref()
                .map_or(0, tessera_net::scenario::InputPlayer::skipped);
            if let Some(log) = &mut self.scenario_log {
                log.after_poll(now, status, counters, closed, input_skipped);
            }
        }
    }

    /// Enters the world one poll after the input script's `at pre` lines were sent (their
    /// answers, such as a created character, have arrived by then).
    fn enter_after_pre_input(&mut self, now: Instant, disconnected: bool) {
        match self.pending_enter.take() {
            Some(_) if disconnected => {}
            Some((name, false)) => self.pending_enter = Some((name, true)),
            Some((name, true)) => {
                let character = name.or_else(|| {
                    self.scenario_input
                        .as_ref()
                        .and_then(|p| p.characters().first().cloned())
                });
                match character {
                    Some(character) => {
                        if let Err(e) = self.send(now, Command::EnterWorld { character }) {
                            tracing::warn!("net: enter world failed: {e}");
                        }
                    }
                    None => {
                        tracing::warn!("net: the account has no characters");
                        self.push_chat(ChatLine::system("This account has no characters."));
                    }
                }
            }
            None => {}
        }
    }

    fn apply(&mut self, now: Instant, ev: &Event) {
        self.events_seen += 1;
        if matches!(
            ev,
            Event::EntityCreated { .. }
                | Event::EntityMoved { .. }
                | Event::EntityDespawned { .. }
                | Event::AppearanceChanged { .. }
                | Event::StaticsCleared
                | Event::Disconnected { .. }
        ) {
            self.queue_entity_event(ev);
        }
        match ev {
            Event::Connected { characters } => {
                let names: Vec<&str> = characters.iter().map(|c| c.name.as_str()).collect();
                tracing::info!(characters = %names.join(","), "net: connected");
                self.show_motd();
                if self.play_pre_input(now) {
                    // A character the `at pre` lines create is entered one poll later;
                    // an unknown one ends the session (§12.2 step 3).
                    self.pending_enter = Some((self.character.clone(), false));
                    return;
                }
                let chosen = match self.character.as_deref() {
                    Some(w) => {
                        let key = name_key(w);
                        let found = characters.iter().find(|c| name_key(&c.name) == key);
                        if found.is_none() {
                            let text = no_such_character(w, &names);
                            tracing::warn!("net: {text}");
                            self.push_chat(ChatLine::system(text));
                            if let Err(e) = self.send(now, Command::Disconnect) {
                                tracing::debug!("net: disconnect not sent: {e}");
                            }
                            return;
                        }
                        found
                    }
                    None => characters.first(),
                };
                if let Some(first) = chosen {
                    let character = first.name.clone();
                    if let Err(e) = self.send(now, Command::EnterWorld { character }) {
                        tracing::warn!("net: enter world failed: {e}");
                    }
                } else {
                    tracing::warn!("net: the account has no characters");
                    self.push_chat(ChatLine::system("This account has no characters."));
                }
            }
            Event::WorldEntered {
                player,
                name,
                transform,
            } => {
                tracing::info!(player = player.0, %name, pos = ?transform.position, "net: world entered");
                self.show_motd();
                self.entered_at = Some(now);
                self.player = Some(*player);
                self.entities.insert(
                    *player,
                    EntityView {
                        kind: EntityKind::Player,
                        name: name.clone(),
                        transform: *transform,
                    },
                );
            }
            Event::EntitySpawned {
                id,
                kind,
                name,
                transform,
            } => {
                tracing::info!(id = id.0, ?kind, %name, pos = ?transform.position, "net: entity spawned");
                self.entities.insert(
                    *id,
                    EntityView {
                        kind: *kind,
                        name: name.clone(),
                        transform: *transform,
                    },
                );
            }
            Event::EntityCreated {
                id,
                kind,
                name,
                location,
                heading,
                tag,
                ..
            } => {
                if tag.is_none() {
                    tracing::info!(id = id.0, ?kind, %name, "net: character created");
                } else {
                    tracing::trace!(id = id.0, ?kind, "net: placed figure created");
                    self.placed.insert(*id);
                }
                self.entities.insert(
                    *id,
                    EntityView {
                        kind: *kind,
                        name: name.clone(),
                        transform: Transform::new(location.position(), *heading),
                    },
                );
            }
            Event::AppearanceChanged { id, appearance } => {
                tracing::debug!(id = id.0, body = ?appearance.body, "net: appearance");
            }
            Event::EntityMoved { id, transform, .. } => {
                self.moves_seen += 1;
                // Movement is chatty; one in 64 at debug, the rest at trace.
                if self.moves_seen % 64 == 1 {
                    tracing::debug!(id = id.0, pos = ?transform.position, moves = self.moves_seen, "net: entity moved");
                } else {
                    tracing::trace!(id = id.0, pos = ?transform.position, "net: entity moved");
                }
                if let Some(e) = self.entities.get_mut(id) {
                    e.transform = *transform;
                }
            }
            Event::EntityDespawned { id } => {
                tracing::debug!(id = id.0, "net: entity despawned");
                self.entities.remove(id);
                self.placed.remove(id);
            }
            Event::ChatMessage {
                channel,
                sender,
                text,
            } => {
                tracing::info!(
                    ?channel,
                    sender = sender.as_deref().unwrap_or("-"),
                    "net chat: {text}"
                );
                self.push_chat(ChatLine::from_channel(channel, sender.as_deref(), text));
            }
            Event::LocationReport {
                id,
                location,
                heading,
                cause,
                ..
            } => {
                if Some(*id) == self.player {
                    tracing::info!(
                        ?cause,
                        region = location.region.0,
                        x = location.x,
                        y = location.y,
                        z = location.z,
                        "net: player location"
                    );
                    self.location_seq += 1;
                    self.location = Some(PlayerLocation {
                        location: *location,
                        heading: *heading,
                        cause: *cause,
                        seq: self.location_seq,
                    });
                }
            }
            Event::StaticSpawned { .. } => {}
            Event::StaticsCleared => {
                for id in std::mem::take(&mut self.placed) {
                    self.entities.remove(&id);
                }
            }
            Event::Disconnected { reason } => {
                tracing::info!(?reason, "net: disconnected");
                let pinned = self.pinned;
                if let Some(text) = self
                    .remote_backend()
                    .and_then(|r| r.failure())
                    .map(|f| describe_failure(f, pinned))
                {
                    tracing::warn!("net: {text}");
                    self.push_chat(ChatLine::system(text));
                }
                self.entities.clear();
                self.placed.clear();
                self.player = None;
                self.location = None;
            }
            other => tracing::debug!(?other, "net: unhandled event"),
        }
    }

    /// Shows the server's name and message of the day (`HelloAck`,
    /// `tessera-protocol.md` §6.2.2; `server-content.md` §6.2 makes it the carrier of the
    /// operator's disclaimer) as system lines, once per session. The decoder has already
    /// sanitised both; the overlay shows them as plain text.
    fn show_motd(&mut self) {
        if self.motd_shown {
            return;
        }
        let Some(info) = self.remote_backend().and_then(|r| r.server_info()).cloned() else {
            return;
        };
        self.motd_shown = true;
        for text in motd_lines(&info.name, &info.motd) {
            tracing::info!("net: server: {text}");
            self.push_chat(ChatLine::system(text));
        }
    }

    /// Disconnect gracefully, polling for at most [`DISCONNECT_GRACE`].
    pub fn shutdown(&mut self) {
        let start = Instant::now();
        if matches!(self.state(), SessionState::Closed) {
            return;
        }
        if let Err(e) = self.send(start, Command::Disconnect) {
            tracing::debug!("net: disconnect not sent: {e}");
            return;
        }
        let mut now = start;
        while !matches!(self.state(), SessionState::Closed) && now - start < DISCONNECT_GRACE {
            self.update(now);
            std::thread::sleep(Duration::from_millis(5));
            now = Instant::now();
        }
        tracing::info!(state = ?self.state(), events = self.events_seen, "net: session shut down");
    }
}

/// The key a character name is matched by (`test-server.md` §6.4): the full Unicode lower
/// case of its NFC form.
#[must_use]
pub fn name_key(name: &str) -> String {
    use unicode_normalization::UnicodeNormalization as _;
    name.trim().nfc().collect::<String>().to_lowercase()
}

/// The system line for a `--character` the account does not have.
fn no_such_character(wanted: &str, names: &[&str]) -> String {
    let list = if names.is_empty() {
        "none".to_owned()
    } else {
        names.join(", ")
    };
    format!("No character '{wanted}' on this account (characters: {list}); not entering the world.")
}

/// The system lines for a server's name and message of the day: `Connected to NAME.`, then
/// the non-empty lines of `motd` (at most [`MAX_MOTD_LINES`]).
#[must_use]
pub fn motd_lines(name: &str, motd: &str) -> Vec<String> {
    let mut out = Vec::new();
    if !name.trim().is_empty() {
        out.push(format!("Connected to {}.", name.trim()));
    }
    out.extend(
        motd.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .take(MAX_MOTD_LINES)
            .map(str::to_owned),
    );
    out
}

/// A user-facing explanation of why a remote session failed (fingerprints are public; the
/// password never appears). `pinned`: the expected key came from a pin (connect string or
/// `--server-fingerprint`), not from the known-servers file (`tessera-protocol.md` §4.5).
#[must_use]
pub fn describe_failure(f: &RemoteFailure, pinned: bool) -> String {
    match f {
        RemoteFailure::Connect {
            failure: ClientFailure::KeyUnknown,
            presented,
        } => match presented {
            Some(fp) => format!(
                "Unknown server key {fp}. Check it with the server operator, then connect with \
                 --server-fingerprint {fp} (or --trust-new-server to record it)."
            ),
            None => "Unknown server key; connect with --server-fingerprint.".to_owned(),
        },
        RemoteFailure::Connect {
            failure: ClientFailure::KeyMismatch,
            presented,
        } => {
            let fp = presented
                .as_ref()
                .map_or_else(|| "?".to_owned(), ToString::to_string);
            if pinned {
                format!(
                    "The server key {fp} does not match the pinned fingerprint; not \
                     connecting. Check the fingerprint with the server operator and update \
                     the pin."
                )
            } else {
                format!(
                    "The server key {fp} does not match the known-servers entry; not \
                     connecting. If the operator changed the key, edit the known-servers file."
                )
            }
        }
        RemoteFailure::Connect { failure, .. } => format!("Connection failed: {failure}."),
        RemoteFailure::Login { code, retry_after } if *retry_after > 0 => {
            format!("Login failed: {code} (retry after {retry_after} s).")
        }
        RemoteFailure::Login { code, .. } => format!("Login failed: {code}."),
        RemoteFailure::EnterWorld { code } => format!("Entering the world failed: {code}."),
        RemoteFailure::Server { code } => format!("The server ended the session: {code}."),
        RemoteFailure::Violation(d) => format!("Protocol violation by the server: {d}."),
        RemoteFailure::Transport(d) => format!("Connection lost: {d}."),
        other => format!("Session failed: {other:?}."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_enters_world_and_shuts_down() {
        let mut now = Instant::now();
        let mut world = World::sandbox(now).expect("sandbox");
        for _ in 0..30 {
            world.update(now);
            now += Duration::from_millis(16);
        }
        assert_eq!(world.state(), SessionState::InWorld);
        assert!(world.player().is_some());
        assert!(!world.entities().is_empty());
        world.say(now, "hello");
        for _ in 0..5 {
            world.update(now);
            now += Duration::from_millis(16);
        }
        assert!(world.take_chat().iter().any(|c| c.text == "hello"));
        world.shutdown();
        assert_eq!(world.state(), SessionState::Closed);
    }

    #[test]
    fn entity_events_are_captured_for_the_walker() {
        let avatar =
            tessera_net::Appearance::new(tessera_net::Body::Character { species: 1, sex: 2 });
        let config = SandboxConfig {
            avatar: Some(avatar.clone()),
            wanderers: 2,
            wanderer_appearances: vec![avatar],
            ..SandboxConfig::default()
        };
        let mut now = Instant::now();
        let mut world = World::sandbox_with(now, config).expect("sandbox");
        world.capture_entity_events(true);
        for _ in 0..20 {
            world.update(now);
            now += Duration::from_millis(16);
        }
        let mut out = Vec::new();
        world.drain_entity_events(&mut out, usize::MAX);
        let count = |f: &dyn Fn(&Event) -> bool| out.iter().filter(|e| f(e)).count();
        assert_eq!(count(&|e| matches!(e, Event::EntityCreated { .. })), 2);
        assert_eq!(count(&|e| matches!(e, Event::AppearanceChanged { .. })), 1);
        assert!(count(&|e| matches!(e, Event::EntityMoved { .. })) > 0);
        assert_eq!(world.entities().len(), 3, "player and two wanderers");
        let mut again = Vec::new();
        world.drain_entity_events(&mut again, usize::MAX);
        assert!(again.is_empty(), "drained");
        world.capture_entity_events(false);
        for _ in 0..10 {
            world.update(now);
            now += Duration::from_millis(16);
        }
        world.drain_entity_events(&mut again, usize::MAX);
        assert!(again.is_empty(), "not captured");
    }

    fn moved(id: u64) -> Event {
        Event::EntityMoved {
            id: EntityId(id),
            transform: Transform::default(),
            velocity: Vec3::default(),
        }
    }

    fn despawned(id: u64) -> Event {
        Event::EntityDespawned { id: EntityId(id) }
    }

    #[test]
    fn entity_event_queue_is_bounded_and_drained_in_slices() {
        let mut world = World::sandbox(Instant::now()).expect("sandbox");
        world.capture_entity_events(true);
        world.queue_cap = 8;
        for i in 0..6 {
            world.queue_entity_event(&moved(i));
        }
        world.queue_entity_event(&despawned(100));
        world.queue_entity_event(&moved(6));
        assert_eq!(world.queued_entity_events(), 8);
        // Full: the queued moves go at once, the removal stays, the new move is queued.
        world.queue_entity_event(&moved(7));
        assert_eq!(world.queued_entity_events(), 2);
        assert_eq!(world.dropped_moves, 7);
        // Non-moves past the cap are never dropped.
        for i in 0..10 {
            world.queue_entity_event(&despawned(200 + i));
        }
        assert_eq!(
            world.queued_entity_events(),
            11,
            "move 7 dropped, removals kept"
        );
        world.queue_entity_event(&moved(8));
        assert_eq!(
            world.queued_entity_events(),
            11,
            "a move past the cap is dropped"
        );
        assert_eq!(world.queued_moves, 0);
        // Slices keep the order.
        let mut out = Vec::new();
        world.drain_entity_events(&mut out, 4);
        assert_eq!(out.len(), 4);
        assert_eq!(out[0], despawned(100));
        assert_eq!(out[1], despawned(200));
        world.drain_entity_events(&mut out, usize::MAX);
        assert_eq!(out.len(), 11);
        assert_eq!(out[10], despawned(209));
        assert_eq!(world.queued_entity_events(), 0);
    }

    fn report(id: EntityId, x: f64, cause: LocationCause) -> Event {
        Event::LocationReport {
            id,
            location: Location::new(tessera_net::RegionId(1), x, 20.0, 0.0),
            heading: 0.0,
            anchor: None,
            cause,
        }
    }

    #[test]
    fn location_seq_survives_a_reconnect() {
        let now = Instant::now();
        let mut world = World::sandbox(now).expect("sandbox");
        let id = EntityId(7);
        world.player = Some(id);
        world.apply(now, &report(id, 10.0, LocationCause::Entered));
        let first = world.player_location().expect("location");
        assert_eq!(first.seq, 1);
        world.apply(
            now,
            &Event::Disconnected {
                reason: tessera_net::DisconnectReason::ClientRequested,
            },
        );
        assert!(world.player_location().is_none());
        world.player = Some(id);
        world.apply(now, &report(id, 30.0, LocationCause::Entered));
        let second = world.player_location().expect("location");
        assert!(second.seq > first.seq, "{second:?} after {first:?}");
        assert_eq!(second.location.x, 30.0);
    }

    fn appearance(id: u64, species: u32) -> Event {
        Event::AppearanceChanged {
            id: EntityId(id),
            appearance: Box::new(tessera_net::Appearance::new(tessera_net::Body::Character {
                species,
                sex: 1,
            })),
        }
    }

    #[test]
    fn appearance_changes_coalesce_per_entity() {
        let mut world = World::sandbox(Instant::now()).expect("sandbox");
        world.capture_entity_events(true);
        world.queue_entity_event(&appearance(1, 10));
        world.queue_entity_event(&moved(2));
        for species in 11..1_000 {
            world.queue_entity_event(&appearance(1, species));
        }
        assert_eq!(world.queued_entity_events(), 2, "replaced in place");
        assert_eq!(world.coalesced_appearances, 989);
        // A removal and re-creation in between: the next change is queued after them.
        world.queue_entity_event(&despawned(1));
        world.queue_entity_event(&appearance(1, 5));
        world.queue_entity_event(&appearance(3, 6));
        let mut out = Vec::new();
        world.drain_entity_events(&mut out, 3);
        assert_eq!(out, [appearance(1, 999), moved(2), despawned(1)]);
        // Still coalesced against the right slot after a partial drain.
        world.queue_entity_event(&appearance(1, 7));
        world.queue_entity_event(&appearance(3, 8));
        out.clear();
        world.drain_entity_events(&mut out, usize::MAX);
        assert_eq!(out, [appearance(1, 7), appearance(3, 8)]);
        // Drained: a new change is queued again, and survives the moves being dropped.
        world.queue_cap = 4;
        world.queue_entity_event(&moved(9));
        world.queue_entity_event(&appearance(1, 1));
        world.queue_entity_event(&moved(9));
        world.queue_entity_event(&moved(9));
        world.queue_entity_event(&despawned(8));
        assert_eq!(world.queued_entity_events(), 2, "moves dropped at the cap");
        world.queue_entity_event(&appearance(1, 2));
        assert_eq!(
            world.queued_entity_events(),
            2,
            "coalesced after the compaction"
        );
        out.clear();
        world.drain_entity_events(&mut out, usize::MAX);
        assert_eq!(out, [appearance(1, 2), despawned(8)]);
    }

    #[test]
    fn a_full_queue_pauses_polling_until_drained() {
        let mut now = Instant::now();
        let mut world = World::sandbox(now).expect("sandbox");
        world.capture_entity_events(true);
        world.queue_cap = 4;
        for i in 0..4 {
            world.queue_entity_event(&despawned(100 + i));
        }
        for _ in 0..10 {
            world.update(now);
            now += Duration::from_millis(16);
        }
        assert_eq!(world.events_seen(), 0, "the session is not read");
        assert_eq!(world.state(), SessionState::Connecting);
        assert_eq!(world.queued_entity_events(), 4);
        let mut out = Vec::new();
        world.drain_entity_events(&mut out, 2);
        for _ in 0..10 {
            world.update(now);
            now += Duration::from_millis(16);
        }
        assert!(world.events_seen() > 0, "reading again after the drain");
        // Bounded by the cap plus one poll's events.
        assert!(world.queued_entity_events() < 4 + 64);
    }

    #[test]
    fn character_names_match_by_key() {
        assert_eq!(name_key("Bramble"), name_key("bramble"));
        assert_eq!(name_key("ÉOWYN"), name_key("éowyn"));
        // NFC: a decomposed é matches the precomposed one.
        assert_eq!(name_key("E\u{301}owyn"), name_key("\u{c9}owyn"));
        assert_ne!(name_key("Bram"), name_key("Bramble"));
        let text = no_such_character("Bramm", &["Aldith", "Bramble"]);
        assert!(
            text.contains("'Bramm'") && text.contains("Aldith, Bramble"),
            "{text}"
        );
        assert!(no_such_character("x", &[]).contains("characters: none"));
    }

    #[test]
    fn motd_lines_are_bounded() {
        assert_eq!(
            motd_lines("Test world", "Testing only.\n\n  Not affiliated.  \n"),
            [
                "Connected to Test world.",
                "Testing only.",
                "Not affiliated."
            ]
        );
        assert!(motd_lines(" ", "").is_empty());
        let many = "line\n".repeat(100);
        assert_eq!(motd_lines("", &many).len(), MAX_MOTD_LINES);
    }

    #[test]
    fn failures_are_described_without_the_password() {
        use tessera_net::remote::{AppErrorCode, EnterWorldFailedCode, LoginFailedCode};
        let fp = Fingerprint::parse(
            "sha256:06e3fd8fda29bb60ab59557de61edb0aecdb231134be30e75b455f8e1b792fa9",
        )
        .unwrap();
        let fps = fp.to_string();
        let connect = |failure, presented| RemoteFailure::Connect { failure, presented };
        let cases: Vec<(RemoteFailure, bool, Vec<&str>, Vec<&str>)> = vec![
            (
                connect(ClientFailure::KeyUnknown, Some(fp)),
                false,
                vec![&fps, "--server-fingerprint", "--trust-new-server"],
                vec![],
            ),
            (
                connect(ClientFailure::KeyUnknown, None),
                false,
                vec!["--server-fingerprint"],
                vec![],
            ),
            (
                connect(ClientFailure::KeyMismatch, Some(fp)),
                false,
                vec![&fps, "known-servers"],
                vec!["pin"],
            ),
            (
                connect(ClientFailure::KeyMismatch, Some(fp)),
                true,
                vec![&fps, "pinned", "operator"],
                vec!["known-servers"],
            ),
            (
                connect(ClientFailure::KeyMismatch, None),
                true,
                vec!["key ?", "pin"],
                vec![],
            ),
            (
                connect(ClientFailure::Timeout, None),
                false,
                vec!["Connection failed", "TIMEOUT"],
                vec![],
            ),
            (
                RemoteFailure::Login {
                    code: LoginFailedCode::RateLimited,
                    retry_after: 30,
                },
                false,
                vec!["Login failed", "RATE_LIMITED", "retry after 30 s"],
                vec![],
            ),
            (
                RemoteFailure::Login {
                    code: LoginFailedCode::BadCredentials,
                    retry_after: 0,
                },
                false,
                vec!["Login failed", "BAD_CREDENTIALS"],
                vec!["retry"],
            ),
            (
                RemoteFailure::EnterWorld {
                    code: EnterWorldFailedCode::UnknownCharacter,
                },
                false,
                vec!["Entering the world failed", "UNKNOWN_CHARACTER"],
                vec![],
            ),
            (
                RemoteFailure::Server {
                    code: AppErrorCode::SlowConsumer,
                },
                false,
                vec!["The server ended the session"],
                vec![],
            ),
            (
                RemoteFailure::Violation("bad frame".into()),
                false,
                vec!["Protocol violation", "bad frame"],
                vec![],
            ),
            (
                RemoteFailure::Transport("reset".into()),
                false,
                vec!["Connection lost", "reset"],
                vec![],
            ),
        ];
        for (failure, pinned, wanted, unwanted) in cases {
            let text = describe_failure(&failure, pinned);
            for w in wanted {
                assert!(text.contains(w), "{failure:?}: {text:?} lacks {w:?}");
            }
            for u in unwanted {
                assert!(!text.contains(u), "{failure:?}: {text:?} has {u:?}");
            }
            assert!(!text.contains("correct horse"), "no password text");
        }
    }

    #[test]
    fn implausible_move_is_corrected() {
        let mut now = Instant::now();
        let mut world = World::sandbox(now).expect("sandbox");
        for _ in 0..30 {
            world.update(now);
            now += Duration::from_millis(16);
        }
        let before = world.player_location().expect("entered");
        let far = Location::new(
            before.location.region,
            before.location.x + 1.0e6,
            before.location.y,
            before.location.z,
        );
        world.send_move(now, far, 0.0, Vec3::ZERO);
        for _ in 0..5 {
            now += Duration::from_millis(16);
            world.update(now);
        }
        let after = world.player_location().expect("location");
        assert!(after.seq > before.seq);
        assert_eq!(after.cause, LocationCause::Corrected);
        assert!((after.location.x - before.location.x).abs() < 1.0);
    }
}
