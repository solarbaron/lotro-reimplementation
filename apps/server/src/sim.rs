// SPDX-License-Identifier: GPL-3.0-or-later
//! The simulation (`test-server.md` §6–§10, §12; `tessera-protocol.md` §7–§11).
//!
//! One [`Sim`] owns all world state: connections as the simulation sees them, accounts in
//! session, characters, world instances, entities (players and wanderers), interest sets,
//! chat, teleports, throttles and persistence scheduling. It runs the fixed 30 Hz tick of
//! §8.1 and talks to the other threads only through bounded channels: it never computes a
//! hash and never touches a file (§14). It is driven synchronously ([`Sim::handle`],
//! [`Sim::tick`]) so tests and fuzz targets can run it without threads.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::net::IpAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::SyncSender;
use std::time::Duration;

use tessera_protocol::codes::{
    AppErrorCode, CharacterCreateFailedCode, ChatRejectedCode, EnterWorldFailedCode,
    LoginFailedCode, TeleportRefusedCode,
};
use tessera_protocol::frame::{encode_datagram, frame_to_vec};
use tessera_protocol::limits::{
    DEFAULT_TICK_RATE, MAX_CHARACTERS, MAX_MOTION_DATAGRAMS_PER_SNAPSHOT, MAX_MOTION_ENTRIES,
    WORLD_BACKLOG_FATAL, WORLD_BACKLOG_GRACE, WORLD_BACKLOG_PAUSE, WORLD_BACKLOG_RESUME,
};
use tessera_protocol::message::{
    ChatChannel, ChatMessage, ChatSend, DestroyReason, EntityAppearance, EntityCreate,
    EntityDestroy, EntityKind, EntityUpdate, LocationCause, LocationReport, Message, MotionBatch,
    MotionEntry, NoticeLevel, ServerNotice, TeleportRefused, TeleportRequest, WorldClock,
    WorldEnter,
};
use tessera_protocol::movement::{
    MoveVerdict, MovementLimits, MovementValidator, block_distance, in_interest, interest_block,
};
use tessera_protocol::rate::SlidingWindow;
use tessera_protocol::server::ServerEvent;
use tessera_protocol::text::sanitise;
use tessera_protocol::types::{
    Appearance, Body, CharacterSummary, Destination, Gait, Location, Motion, Pose, Vec3,
};
use tessera_protocol::value::{Emote, Property, Stance, Value, pid};

use crate::accounts::{self, AccountStore};
use crate::characters::{
    CharacterFile, CharacterRecord, StoredAppearance, StoredPosition, WorldState,
};
use crate::config::{Config, DuplicateLogin, TeleportPolicy, YellScope};
use crate::content::{Content, Spawn, WandererLook, compass_from_heading};
use crate::datadir::{self, DataDir};
use crate::names;
use crate::shared::{
    ConnId, ConnShared, HashJob, HashOutcome, HashResult, Outbound, SessionCmd, SimControl,
    SimInput, Stats, StopReport, WriteJob, WriterMsg,
};
use crate::workers::HASH_QUEUE;

const SECOND: u64 = 1_000_000;
/// Control messages processed per tick over all connections (§8.1 step 1).
pub const CONTROL_PER_TICK: usize = 256;
/// Fixed accounting per connection (§10.2).
const CONNECTION_STATE_BYTES: usize = 64 * 1024;
/// Accounted size of one queued inbound control event.
const INBOUND_EVENT_BYTES: usize = 1024;
/// Chat-stream backlog limits (§10.1).
const CHAT_BACKLOG_SOFT: usize = 256 * 1024;
const CHAT_BACKLOG_HARD: usize = 1024 * 1024;
const CHAT_BACKLOG_GRACE: u64 = 30 * SECOND;
/// Non-looping emotes end after this long (`tessera-protocol.md` §7.3).
const EMOTE_TIMEOUT: u64 = 60 * SECOND;
/// Saves after a teleport wait this long (one write per burst, §12.2).
const TELEPORT_SAVE_DELAY: u64 = 5 * SECOND;
/// Movement that counts as activity (§8.1).
const ACTIVE_DISTANCE: f64 = 0.1;
/// Movement that makes a periodic save worthwhile (§12.2).
const SAVE_DISTANCE: f64 = 1.0;
/// A `MotionBatch` refresh is due after this long (§8.5).
const MOTION_REFRESH: u64 = SECOND;
/// Snapshots repeated after an entity comes to rest (§8.5).
const REST_SNAPSHOTS: u32 = 3;
/// Spawn names within this distance are named in location descriptions (§8.5).
const NEARBY_SPAWN: f64 = 160.0;
/// A bigger vertical allowance used during the settling grace (§8.3).
const GRACE_VERTICAL: f64 = 1.0e9;
/// Most account-lockout entries kept (failures are counted for every canonical name, known
/// or not, so the table must not grow with invented names).
const MAX_LOCKOUTS: usize = 65_536;

fn micros(d: Duration) -> u64 {
    u64::try_from(d.as_micros()).unwrap_or(u64::MAX)
}

/// What the simulation needs at start.
#[derive(Debug)]
pub struct SimParts {
    /// The configuration.
    pub config: Arc<Config>,
    /// The content.
    pub content: Arc<Content>,
    /// The data directory (character and world-state paths).
    pub data_dir: DataDir,
    /// The accounts.
    pub accounts: Arc<AccountStore>,
    /// Every account's characters.
    pub characters: BTreeMap<String, CharacterFile>,
    /// Saved game clocks.
    pub world_state: WorldState,
    /// Login verifications.
    pub hash_jobs: SyncSender<HashJob>,
    /// File writes.
    pub writer: SyncSender<WriterMsg>,
    /// Counters.
    pub stats: Arc<Stats>,
}

/// A token bucket with fractional refill.
#[derive(Debug, Clone)]
struct Bucket {
    tokens: f64,
    capacity: f64,
    /// Seconds per token.
    refill_s: f64,
    last: u64,
}

impl Bucket {
    fn new(capacity: f64, refill_s: f64, now: u64) -> Self {
        Self {
            tokens: capacity,
            capacity,
            refill_s: refill_s.max(1e-6),
            last: now,
        }
    }

    fn refill(&mut self, now: u64) {
        let dt = now.saturating_sub(self.last) as f64 / SECOND as f64;
        self.last = self.last.max(now);
        self.tokens = (self.tokens + dt / self.refill_s).min(self.capacity);
    }

    fn take(&mut self, now: u64) -> bool {
        self.refill(now);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Whether the bucket would be full at `now` (then it is the same as a new one).
    fn full_at(&self, now: u64) -> bool {
        let dt = now.saturating_sub(self.last) as f64 / SECOND as f64;
        self.tokens + dt / self.refill_s >= self.capacity
    }

    /// Seconds until the next token, rounded up, at least 1.
    fn wait_secs(&self) -> u16 {
        let s = ((1.0 - self.tokens).max(0.0) * self.refill_s).ceil();
        (s as u16).max(1)
    }
}

#[derive(Debug, Clone)]
struct Lockout {
    failures: u32,
    until: u64,
    duration_s: u64,
    last: u64,
}

#[derive(Debug, Clone)]
struct Wander {
    center_x: f64,
    center_y: f64,
    z: f64,
    radius: f64,
    speed: f64,
    start_angle: f64,
    t0: u64,
}

impl Wander {
    /// Position, velocity and heading at server time `t` (§5.4).
    fn state(&self, t: u64) -> (f64, f64, Vec3, f32) {
        let dt = t.saturating_sub(self.t0) as f64 / SECOND as f64;
        let omega = self.speed / self.radius;
        let phi = self.start_angle + omega * dt;
        let x = self.center_x + self.radius * phi.cos();
        let y = self.center_y + self.radius * phi.sin();
        let vx = -self.speed * phi.sin();
        let vy = self.speed * phi.cos();
        let heading = tessera_protocol::types::reduce_heading(vy.atan2(vx) as f32);
        (x, y, Vec3::new(vx as f32, vy as f32, 0.0), heading)
    }
}

#[derive(Debug, Clone)]
struct Entity {
    id: u64,
    kind: EntityKind,
    name: String,
    instance: u32,
    pose: Pose,
    velocity: Vec3,
    motion: Motion,
    appearance: Appearance,
    stance: Stance,
    emote: Option<Emote>,
    owner: Option<ConnId>,
    block: (u8, u8),
    motion_version: u64,
    dirty_props: BTreeSet<u16>,
    dirty_appearance: bool,
    wander: Option<Wander>,
    removed: bool,
}

impl Entity {
    fn region(&self) -> u16 {
        self.pose.location.region
    }

    fn update_block(&mut self) {
        self.block = interest_block(&self.pose.location, self.pose.anchor.as_ref());
    }

    fn properties(&self) -> Vec<Property> {
        let mut props = Vec::new();
        if self.stance != Stance::Relaxed {
            props.push(Property::new(pid::STANCE, self.stance.to_property()));
        }
        if let Some(e) = self.emote {
            props.push(Property::new(pid::EMOTE, e.to_property()));
        }
        props
    }

    fn property_value(&self, id: u16) -> Value {
        match id {
            pid::STANCE if self.stance != Stance::Relaxed => self.stance.to_property(),
            pid::EMOTE => self.emote.map_or(Value::None, Emote::to_property),
            _ => Value::None,
        }
    }

    fn at_rest(&self) -> bool {
        self.motion.gait == Gait::Standing && self.velocity == Vec3::ZERO
    }
}

#[derive(Debug, Clone, Default)]
struct Known {
    sent_motion_version: u64,
    last_sent: u64,
    rest_left: u32,
    pending_props: BTreeSet<u16>,
    pending_appearance: bool,
}

#[derive(Debug, Clone)]
struct PendingTeleport {
    request_id: u32,
    destination: Destination,
    received: u64,
}

#[derive(Debug)]
struct Conn {
    shared: Arc<ConnShared>,
    queue: VecDeque<ServerEvent>,
    max_entities: usize,
    account: Option<String>,
    login_pending: bool,
    create_pending: Option<u64>,
    entity: Option<u64>,
    character_key: Option<String>,
    known: HashMap<u64, Known>,
    validator: Option<MovementValidator>,
    grace_until: u64,
    chat_bucket: Bucket,
    chat_rejections: SlidingWindow,
    flood_limit: usize,
    last_activity: u64,
    active_pos: Location,
    teleport: Option<PendingTeleport>,
    last_teleport: Option<u64>,
    save_at: Option<u64>,
    saved_pos: Location,
    needs_save: bool,
    world_paused: bool,
    world_over_since: Option<u64>,
    chat_over_since: Option<u64>,
    dead: bool,
}

#[derive(Debug, Clone)]
struct Instance {
    number: u32,
    players: u32,
    clock_base: f64,
    clock_t0: u64,
    rate: f32,
    first_player_groups: Vec<usize>,
    next_wanderer: u32,
}

impl Instance {
    fn game_clock(&self, now: u64) -> f64 {
        let dt = now.saturating_sub(self.clock_t0) as f64 / SECOND as f64;
        (self.clock_base + f64::from(self.rate) * dt).max(0.0)
    }
}

#[derive(Debug, Clone)]
struct PendingCreate {
    conn: ConnId,
    account: String,
    name_key: String,
}

/// The simulation.
#[derive(Debug)]
pub struct Sim {
    config: Arc<Config>,
    content: Arc<Content>,
    data_dir: DataDir,
    accounts: Arc<AccountStore>,
    accounts_generation: u64,
    characters: BTreeMap<String, CharacterFile>,
    /// Character name key → account.
    name_index: HashMap<String, String>,
    conns: HashMap<ConnId, Conn>,
    conn_order: Vec<ConnId>,
    online: HashMap<String, ConnId>,
    players_by_key: HashMap<String, u64>,
    entities: HashMap<u64, Entity>,
    next_entity: u64,
    instances: Vec<Instance>,
    addr_buckets: HashMap<IpAddr, Bucket>,
    lockouts: HashMap<String, Lockout>,
    hash_inflight: usize,
    hash_jobs: SyncSender<HashJob>,
    writer: SyncSender<WriterMsg>,
    pending_creates: HashMap<u64, PendingCreate>,
    next_token: u64,
    stats: Arc<Stats>,
    tick: u64,
    next_save: u64,
    next_stats: u64,
    last_overrun_log: u64,
    limits: MovementLimits,
    grace_limits: MovementLimits,
    stopped: bool,
    rr_start: usize,
}

impl Sim {
    /// A simulation at server time `now`.
    #[must_use]
    pub fn new(parts: SimParts, now: u64) -> Self {
        let SimParts {
            config,
            content,
            data_dir,
            accounts,
            characters,
            world_state,
            hash_jobs,
            writer,
            stats,
        } = parts;
        let m = &config.movement;
        // §4.5 and §8.3: the limits do not depend on the walk and run speeds; the descent
        // credit lets slides down steep faces pass.
        let limits = MovementLimits {
            horizontal_speed: m.max_horizontal_speed,
            up_speed: m.max_up_speed,
            down_speed: m.max_down_speed,
            window: Duration::from_secs_f64(m.window_s),
            descent_credit: Some(m.slide_reserve),
            ..MovementLimits::default()
        };
        // During the settling grace descents are not checked, so they earn no credit
        // either (otherwise a claimed descent would lift the horizontal check).
        let grace_limits = MovementLimits {
            up_speed: GRACE_VERTICAL,
            down_speed: GRACE_VERTICAL,
            descent_credit: None,
            ..limits
        };
        let mut name_index = HashMap::new();
        for (account, file) in &characters {
            for c in &file.characters {
                name_index.insert(names::name_key(&c.name), account.clone());
            }
        }
        let accounts_generation = accounts.generation();
        let mut sim = Self {
            config: Arc::clone(&config),
            content: Arc::clone(&content),
            data_dir,
            accounts,
            accounts_generation,
            characters,
            name_index,
            conns: HashMap::new(),
            conn_order: Vec::new(),
            online: HashMap::new(),
            players_by_key: HashMap::new(),
            entities: HashMap::new(),
            next_entity: 1,
            instances: Vec::new(),
            addr_buckets: HashMap::new(),
            lockouts: HashMap::new(),
            hash_inflight: 0,
            hash_jobs,
            writer,
            pending_creates: HashMap::new(),
            next_token: 1,
            stats,
            tick: 0,
            next_save: now + u64::from(config.world.save_interval_s) * SECOND,
            next_stats: now + u64::from(config.log.stats_interval_s.max(1)) * SECOND,
            last_overrun_log: 0,
            limits,
            grace_limits,
            stopped: false,
            rr_start: 0,
        };
        for n in 1..=config.world.instances {
            let (base, t0) = match world_state.clocks.get(&n) {
                Some(g) => (*g, now),
                None => (content.clock_start, now),
            };
            let mut inst = Instance {
                number: n,
                players: 0,
                clock_base: base,
                clock_t0: t0,
                rate: content.clock_rate,
                first_player_groups: Vec::new(),
                next_wanderer: 1,
            };
            for (gi, g) in content.wanderers.iter().enumerate() {
                if let WandererLook::FirstPlayer = g.look {
                    inst.first_player_groups.push(gi);
                }
            }
            sim.instances.push(inst);
            for (gi, g) in content.wanderers.iter().enumerate() {
                if let WandererLook::Fixed(a) = &g.look {
                    let a = a.clone();
                    sim.create_wanderers(n, gi, &a, now);
                }
            }
        }
        sim.publish_stats(now);
        sim
    }

    /// Whether [`SimControl::Stop`] was handled.
    #[must_use]
    pub fn stopped(&self) -> bool {
        self.stopped
    }

    /// The tick last run.
    #[must_use]
    pub fn tick_number(&self) -> u64 {
        self.tick
    }

    // ------------------------------------------------------------------ helpers

    fn send(&self, conn: ConnId, out: Outbound) -> bool {
        match self.conns.get(&conn) {
            Some(c) => c.shared.send(out),
            None => false,
        }
    }

    fn cmd(&self, conn: ConnId, cmd: SessionCmd) {
        self.send(conn, Outbound::Cmd(cmd));
    }

    fn send_world(&self, conn: ConnId, msg: &Message) -> usize {
        let Some(c) = self.conns.get(&conn) else {
            return 0;
        };
        match frame_to_vec(msg) {
            Ok(bytes) => {
                let n = bytes.len();
                c.shared.world_unsent.fetch_add(n, Ordering::Relaxed);
                if !c.shared.send(Outbound::World(bytes)) {
                    c.shared.world_unsent.fetch_sub(n, Ordering::Relaxed);
                    return 0;
                }
                n
            }
            Err(e) => {
                tracing::error!(error = %e, ty = %msg.message_type(), "world message not encodable");
                0
            }
        }
    }

    fn send_world_bytes(&self, conn: ConnId, bytes: Vec<u8>) {
        let Some(c) = self.conns.get(&conn) else {
            return;
        };
        let n = bytes.len();
        c.shared.world_unsent.fetch_add(n, Ordering::Relaxed);
        if !c.shared.send(Outbound::World(bytes)) {
            c.shared.world_unsent.fetch_sub(n, Ordering::Relaxed);
        }
    }

    fn send_chat_frame(&self, conn: ConnId, msg: &Message) {
        let Some(c) = self.conns.get(&conn) else {
            return;
        };
        match frame_to_vec(msg) {
            Ok(bytes) => {
                let n = bytes.len();
                c.shared.chat_unsent.fetch_add(n, Ordering::Relaxed);
                if !c.shared.send(Outbound::Chat(bytes)) {
                    c.shared.chat_unsent.fetch_sub(n, Ordering::Relaxed);
                }
            }
            Err(e) => tracing::error!(error = %e, "chat message not encodable"),
        }
    }

    fn system_line(&self, conn: ConnId, text: &str, now: u64) {
        let mut t = sanitise(text).into_owned();
        t.truncate(floor(&t, tessera_protocol::limits::MAX_TEXT_LEN));
        if t.is_empty() {
            return;
        }
        let msg = Message::ChatMessage(ChatMessage {
            server_time: now,
            channel: ChatChannel::System,
            echo: false,
            sender_entity: 0,
            sender: String::new(),
            text: t,
        });
        self.send_chat_frame(conn, &msg);
    }

    fn new_entity_id(&mut self) -> u64 {
        let id = self.next_entity;
        self.next_entity = self.next_entity.wrapping_add(1).max(1);
        id
    }

    fn next_token(&mut self) -> u64 {
        let t = self.next_token;
        self.next_token += 1;
        t
    }

    /// Queues a file write. With `wait`, waits for room in the writer queue instead of
    /// giving up when it is full (saves that are never skipped: leaving the world and
    /// shutdown, §10.1, §12.1). The writer never blocks on the simulation, so waiting cannot
    /// deadlock.
    fn write_file(
        &self,
        path: std::path::PathBuf,
        contents: String,
        token: Option<u64>,
        wait: bool,
    ) -> bool {
        let job = WriterMsg::Write(WriteJob {
            path,
            contents: contents.into_bytes(),
            mode: 0o600,
            token,
        });
        // Counted first: the writer decrements as soon as it takes the job.
        self.stats.writer_jobs.fetch_add(1, Ordering::Relaxed);
        let ok = if wait {
            self.writer.send(job).is_ok()
        } else {
            self.writer.try_send(job).is_ok()
        };
        if !ok {
            saturating_dec(&self.stats.writer_jobs);
        }
        ok
    }

    fn save_account(&self, account: &str, token: Option<u64>, wait: bool) -> bool {
        let Some(file) = self.characters.get(account) else {
            return false;
        };
        let ok = self.write_file(
            self.data_dir.character_path(account),
            file.to_toml(),
            token,
            wait,
        );
        if !ok {
            if wait {
                tracing::error!(account = %account, "the writer stopped: character save failed");
            } else {
                tracing::warn!("writer queue full: character save skipped");
            }
        }
        ok
    }

    fn world_state(&self, now: u64) -> WorldState {
        let mut ws = WorldState::default();
        for i in &self.instances {
            ws.clocks.insert(i.number, i.game_clock(now));
        }
        ws
    }

    fn summaries(&self, account: &str) -> Vec<CharacterSummary> {
        self.characters
            .get(account)
            .map(|f| {
                f.characters
                    .iter()
                    .take(MAX_CHARACTERS)
                    .map(|c| CharacterSummary {
                        name: c.name.clone(),
                        body: match &c.appearance {
                            StoredAppearance::Valid(a) => a.body,
                            StoredAppearance::Invalid(_) => Body::Character { species: 0, sex: 0 },
                        },
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    // ------------------------------------------------------------------ input

    /// Handles one input. Lifecycle events, hash results and write results are acted on at
    /// once; control events are queued for the next tick (§8.1 step 1).
    pub fn handle(&mut self, input: SimInput, now: u64) {
        match input {
            SimInput::Opened(shared) => {
                let id = shared.id;
                let cfg = &self.config.chat;
                let conn = Conn {
                    shared,
                    queue: VecDeque::new(),
                    max_entities: 0,
                    account: None,
                    login_pending: false,
                    create_pending: None,
                    entity: None,
                    character_key: None,
                    known: HashMap::new(),
                    validator: None,
                    grace_until: 0,
                    chat_bucket: Bucket::new(f64::from(cfg.bucket_capacity), cfg.refill_s, now),
                    chat_rejections: SlidingWindow::new(
                        cfg.flood_rejections,
                        Duration::from_secs(u64::from(cfg.flood_window_s)),
                    ),
                    flood_limit: usize::try_from(cfg.flood_rejections).unwrap_or(20),
                    last_activity: now,
                    active_pos: Location::default(),
                    teleport: None,
                    last_teleport: None,
                    save_at: None,
                    saved_pos: Location::default(),
                    needs_save: false,
                    world_paused: false,
                    world_over_since: None,
                    chat_over_since: None,
                    dead: false,
                };
                self.conns.insert(id, conn);
                self.conn_order.push(id);
            }
            SimInput::Event(id, ev) => {
                if let Some(c) = self.conns.get_mut(&id) {
                    c.queue.push_back(ev);
                }
            }
            SimInput::Closed(id) => self.on_closed(id, now),
            SimInput::Hash(r) => self.on_hash(r, now),
            SimInput::WriteDone { token, ok } => self.on_write_done(token, ok),
            SimInput::Control(c) => match c {
                SimControl::ShutdownNotice { seconds } => self.shutdown_notice(seconds),
                SimControl::Stop(reply) => {
                    let r = self.stop(now);
                    let _ = reply.send(r);
                }
            },
        }
    }

    fn on_closed(&mut self, id: ConnId, now: u64) {
        if self.conns.contains_key(&id) {
            self.finish_queued(id, now);
            self.leave_world(id);
            self.release_account(id);
            if let Some(c) = self.conns.remove(&id) {
                c.shared.inbound_pending.store(0, Ordering::Release);
                c.shared.world_unsent.store(0, Ordering::Relaxed);
                c.shared.chat_unsent.store(0, Ordering::Relaxed);
                c.shared.control_unsent.store(0, Ordering::Relaxed);
            }
            self.conn_order.retain(|c| *c != id);
            self.pending_creates.retain(|_, p| p.conn != id);
        }
    }

    /// Applies what a connection sent before it closed and the simulation has not processed
    /// yet: queued events are handled for the next tick (§8.1 step 1), so a `ChatSend`,
    /// `EmoteSet`, `StanceSet` or `AppearanceSet` sent just before `Disconnect` still takes
    /// effect, and the newest `MoveInput` is validated before the character is saved (§12.2
    /// "last accepted" position). Requests whose answer can no longer arrive (login, creation,
    /// world entry, teleports, location queries) are dropped.
    fn finish_queued(&mut self, id: ConnId, now: u64) {
        let Some(c) = self.conns.get_mut(&id) else {
            return;
        };
        if c.dead {
            c.queue.clear();
            return;
        }
        let queue = std::mem::take(&mut c.queue);
        for ev in queue {
            if let Some(c) = self.conns.get(&id) {
                saturating_dec(&c.shared.inbound_pending);
            }
            if self.conns.get(&id).is_none_or(|c| c.dead) {
                continue;
            }
            match ev {
                ServerEvent::Chat(_)
                | ServerEvent::Emote(_)
                | ServerEvent::Stance(_)
                | ServerEvent::Appearance(_)
                | ServerEvent::Move(_) => self.on_event(id, ev, now),
                _ => {}
            }
        }
        if self.conns.get(&id).is_some_and(|c| !c.dead) {
            self.move_one(id, now);
        }
    }

    fn release_account(&mut self, id: ConnId) {
        let Some(c) = self.conns.get_mut(&id) else {
            return;
        };
        if let Some(account) = c.account.take()
            && self.online.get(&account) == Some(&id)
        {
            self.online.remove(&account);
        }
    }

    /// Ends a session from the server side: leaves the world, releases the account and
    /// sends `Disconnect(code)`.
    fn disconnect(&mut self, id: ConnId, code: AppErrorCode, text: &str) {
        let Some(c) = self.conns.get(&id) else {
            return;
        };
        if c.dead {
            return;
        }
        tracing::debug!(conn = id, code = %code, "disconnecting");
        if code == AppErrorCode::SlowConsumer {
            self.stats.slow_consumers.fetch_add(1, Ordering::Relaxed);
        }
        self.leave_world(id);
        self.release_account(id);
        if let Some(c) = self.conns.get_mut(&id) {
            c.dead = true;
            c.queue.clear();
            if !c
                .shared
                .send(Outbound::Cmd(SessionCmd::Disconnect(code, text.to_owned())))
            {
                c.shared.kill(code);
            }
        }
    }

    // ------------------------------------------------------------------ login

    fn reject_login(&self, id: ConnId, code: LoginFailedCode, retry_after: u16) {
        self.cmd(id, SessionCmd::RejectLogin { code, retry_after });
    }

    fn on_login(
        &mut self,
        id: ConnId,
        account: String,
        password: String,
        register: bool,
        now: u64,
    ) {
        let addr = match self.conns.get(&id) {
            Some(c) if !c.login_pending && c.account.is_none() => c.shared.addr.ip(),
            _ => {
                accounts::wipe(password);
                return;
            }
        };
        let Some(canonical) = names::canonical_account(&account) else {
            accounts::wipe(password);
            self.reject_login(id, LoginFailedCode::InvalidAccountName, 0);
            return;
        };
        // 1. Address budget.
        let per_min = f64::from(self.config.accounts.attempts_per_address_per_min);
        let bucket = self
            .addr_buckets
            .entry(addr)
            .or_insert_with(|| Bucket::new(per_min, 60.0 / per_min, now));
        if !bucket.take(now) {
            let wait = bucket.wait_secs();
            accounts::wipe(password);
            tracing::debug!(%addr, "login refused: address budget");
            self.reject_login(id, LoginFailedCode::RateLimited, wait);
            return;
        }
        // 2. Account lockout.
        if let Some(l) = self.lockouts.get(&canonical)
            && l.until > now
        {
            let secs = (l.until - now)
                .div_ceil(SECOND)
                .clamp(1, u64::from(u16::MAX));
            accounts::wipe(password);
            self.reject_login(id, LoginFailedCode::RateLimited, secs as u16);
            return;
        }
        if register {
            if !self.config.server.registration {
                accounts::wipe(password);
                self.reject_login(id, LoginFailedCode::RegistrationClosed, 0);
                return;
            }
            if self.accounts.get(&canonical).is_some() {
                accounts::wipe(password);
                self.reject_login(id, LoginFailedCode::AccountExists, 0);
                return;
            }
            if !accounts::password_acceptable(
                &password,
                &canonical,
                self.config.accounts.password_min_bytes,
            ) {
                accounts::wipe(password);
                self.reject_login(id, LoginFailedCode::PasswordRejected, 0);
                return;
            }
        }
        // 3. Queue.
        let workers = usize::try_from(self.config.accounts.hash_workers).unwrap_or(1);
        if self.hash_inflight >= workers + HASH_QUEUE {
            accounts::wipe(password);
            self.reject_login(id, LoginFailedCode::RateLimited, 5);
            return;
        }
        let job = HashJob {
            conn: id,
            account: canonical,
            password,
            register,
        };
        match self.hash_jobs.try_send(job) {
            Ok(()) => {
                self.hash_inflight += 1;
                self.stats.hash_jobs.fetch_add(1, Ordering::Relaxed);
                if let Some(c) = self.conns.get_mut(&id) {
                    c.login_pending = true;
                }
            }
            Err(e) => {
                let job = match e {
                    std::sync::mpsc::TrySendError::Full(j)
                    | std::sync::mpsc::TrySendError::Disconnected(j) => j,
                };
                accounts::wipe(job.password);
                self.reject_login(id, LoginFailedCode::RateLimited, 5);
            }
        }
    }

    fn on_hash(&mut self, r: HashResult, now: u64) {
        self.hash_inflight = self.hash_inflight.saturating_sub(1);
        let id = r.conn;
        let addr = match self.conns.get_mut(&id) {
            Some(c) if !c.dead => {
                c.login_pending = false;
                c.shared.addr
            }
            _ => return,
        };
        let known = self.accounts.get(&r.account);
        match r.outcome {
            HashOutcome::Error => {
                self.reject_login(id, LoginFailedCode::ServerError, 0);
            }
            HashOutcome::AccountExists => {
                self.reject_login(id, LoginFailedCode::AccountExists, 0);
            }
            HashOutcome::RegistrationFull => {
                self.reject_login(id, LoginFailedCode::RegistrationClosed, 0);
            }
            HashOutcome::BadCredentials => {
                // Failures are counted and locked out per canonical name whether the account
                // exists or not: otherwise a lockout (RATE_LIMITED) would tell a known name
                // from an unknown one (`tessera-protocol.md` §4.7).
                self.count_failure(&r.account, now);
                if known.is_some() {
                    tracing::info!(%addr, account = %r.account, "login failed");
                } else {
                    tracing::info!(%addr, "login failed: unknown account");
                }
                self.reject_login(id, LoginFailedCode::BadCredentials, 0);
            }
            HashOutcome::Verified => {
                let Some(acct) = self.accounts.get(&r.account) else {
                    self.reject_login(id, LoginFailedCode::ServerError, 0);
                    return;
                };
                if acct.disabled {
                    self.reject_login(id, LoginFailedCode::AccountDisabled, 0);
                    return;
                }
                self.lockouts.remove(&r.account);
                if let Some(&old) = self.online.get(&r.account)
                    && old != id
                {
                    match self.config.server.duplicate_login {
                        DuplicateLogin::Refuse => {
                            self.reject_login(id, LoginFailedCode::AlreadyOnline, 0);
                            return;
                        }
                        DuplicateLogin::Replace => {
                            self.disconnect(old, AppErrorCode::LoggedInElsewhere, "");
                        }
                    }
                }
                if r.register {
                    tracing::info!(%addr, account = %r.account, "account registered");
                }
                tracing::info!(%addr, account = %r.account, "login");
                self.online.insert(r.account.clone(), id);
                if let Some(c) = self.conns.get_mut(&id) {
                    c.account = Some(r.account.clone());
                    c.last_activity = now;
                }
                let characters = self.summaries(&r.account);
                self.cmd(
                    id,
                    SessionCmd::AcceptLogin {
                        account: r.account,
                        characters,
                    },
                );
            }
        }
    }

    fn count_failure(&mut self, account: &str, now: u64) {
        if !self.lockouts.contains_key(account) && self.lockouts.len() >= MAX_LOCKOUTS {
            self.lockouts.retain(|_, l| l.until > now);
            if self.lockouts.len() >= MAX_LOCKOUTS {
                tracing::warn!("lockout table full: failure not counted");
                return;
            }
        }
        let cfg = &self.config.accounts;
        let l = self.lockouts.entry(account.to_owned()).or_insert(Lockout {
            failures: 0,
            until: 0,
            duration_s: 0,
            last: now,
        });
        l.failures = l.failures.saturating_add(1);
        l.last = now;
        if l.failures >= cfg.lockout_after {
            l.duration_s = if l.duration_s == 0 {
                u64::from(cfg.lockout_base_s)
            } else {
                (l.duration_s * 2).min(u64::from(cfg.lockout_max_s))
            };
            l.until = now + l.duration_s * SECOND;
        }
    }

    // ------------------------------------------------------------------ characters

    fn on_create(&mut self, id: ConnId, name: String, appearance: Appearance) {
        let Some(account) = self.conns.get(&id).and_then(|c| c.account.clone()) else {
            return;
        };
        let reject = |s: &Self, code| s.cmd(id, SessionCmd::RejectCharacter(code));
        if !self.config.server.character_creation {
            reject(self, CharacterCreateFailedCode::Disabled);
            return;
        }
        let Some(nfc) = names::character_name(&name) else {
            reject(self, CharacterCreateFailedCode::NameInvalid);
            return;
        };
        let key = names::name_key(&nfc);
        if self.name_index.contains_key(&key)
            || self.pending_creates.values().any(|p| p.name_key == key)
        {
            reject(self, CharacterCreateFailedCode::NameTaken);
            return;
        }
        let count = self
            .characters
            .get(&account)
            .map_or(0, |f| f.characters.len());
        let max = usize::try_from(self.config.accounts.max_characters)
            .unwrap_or(MAX_CHARACTERS)
            .min(MAX_CHARACTERS);
        if count >= max {
            reject(self, CharacterCreateFailedCode::TooMany);
            return;
        }
        if matches!(appearance.body, Body::Object { .. })
            && !self.config.accounts.allow_object_bodies
        {
            reject(self, CharacterCreateFailedCode::AppearanceRefused);
            return;
        }
        let Some(spawn) = self.content.default_spawn().cloned() else {
            reject(self, CharacterCreateFailedCode::ServerError);
            return;
        };
        let record = CharacterRecord {
            name: nfc.clone(),
            created: datadir::utc_rfc3339(std::time::SystemTime::now()),
            instance: 0,
            position: Some(StoredPosition {
                location: spawn.location,
                heading: spawn.heading,
                anchor: spawn.anchor,
            }),
            appearance: StoredAppearance::Valid(appearance),
        };
        let file = self
            .characters
            .entry(account.clone())
            .or_insert_with(|| CharacterFile {
                account: account.clone(),
                characters: Vec::new(),
            });
        file.characters.push(record);
        self.name_index.insert(key.clone(), account.clone());
        self.stats
            .character_files
            .store(self.characters.len(), Ordering::Relaxed);
        let token = self.next_token();
        if self.save_account(&account, Some(token), false) {
            self.pending_creates.insert(
                token,
                PendingCreate {
                    conn: id,
                    account,
                    name_key: key,
                },
            );
            if let Some(c) = self.conns.get_mut(&id) {
                c.create_pending = Some(token);
            }
        } else {
            self.undo_create(&account, &key);
            reject(self, CharacterCreateFailedCode::ServerError);
        }
    }

    fn undo_create(&mut self, account: &str, key: &str) {
        if let Some(f) = self.characters.get_mut(account) {
            f.characters.retain(|c| names::name_key(&c.name) != key);
        }
        self.name_index.remove(key);
    }

    fn on_write_done(&mut self, token: u64, ok: bool) {
        let Some(p) = self.pending_creates.remove(&token) else {
            return;
        };
        if let Some(c) = self.conns.get_mut(&p.conn) {
            c.create_pending = None;
        }
        if ok {
            let list = self.summaries(&p.account);
            self.cmd(p.conn, SessionCmd::CharacterCreated(list));
        } else {
            self.undo_create(&p.account, &p.name_key);
            self.cmd(
                p.conn,
                SessionCmd::RejectCharacter(CharacterCreateFailedCode::ServerError),
            );
        }
    }

    // ------------------------------------------------------------------ world entry

    fn movement_props(&self) -> Vec<Property> {
        vec![
            Property::new(
                pid::WALK_SPEED,
                Value::Float32(self.config.movement.walk_speed),
            ),
            Property::new(
                pid::RUN_SPEED,
                Value::Float32(self.config.movement.run_speed),
            ),
        ]
    }

    fn on_enter(&mut self, id: ConnId, name: String, now: u64) {
        let Some(account) = self.conns.get(&id).and_then(|c| c.account.clone()) else {
            return;
        };
        let reject = |s: &Self, code| s.cmd(id, SessionCmd::RejectEnterWorld(code));
        let key = names::name_key(&name);
        let Some(record) = self
            .characters
            .get(&account)
            .and_then(|f| {
                f.characters
                    .iter()
                    .find(|c| names::name_key(&c.name) == key)
            })
            .cloned()
        else {
            reject(self, EnterWorldFailedCode::UnknownCharacter);
            return;
        };
        if self.players_by_key.contains_key(&key) {
            reject(self, EnterWorldFailedCode::CharacterBusy);
            return;
        }
        let appearance = match &record.appearance {
            StoredAppearance::Valid(a) => a.clone(),
            StoredAppearance::Invalid(_) => {
                tracing::error!(
                    file = %self.data_dir.character_path(&account).display(),
                    "a character's stored appearance fails the protocol's limits; world entry refused"
                );
                reject(self, EnterWorldFailedCode::ServerError);
                return;
            }
        };
        let cap = self.config.world.instance_capacity;
        let instance = {
            let stored = self
                .instances
                .iter()
                .find(|i| i.number == record.instance && i.players < cap);
            stored
                .or_else(|| self.instances.iter().find(|i| i.players < cap))
                .map(|i| i.number)
        };
        let Some(instance) = instance else {
            reject(self, EnterWorldFailedCode::NoInstance);
            return;
        };
        let Some(spawn) = self.content.default_spawn().cloned() else {
            reject(self, EnterWorldFailedCode::ServerError);
            return;
        };
        let pose = match &record.position {
            Some(p) => Pose {
                location: p.location,
                heading: p.heading,
                anchor: p.anchor,
            },
            None => spawn_pose(&spawn),
        };
        // The settling grace applies whenever the epoch starts at a ground spawn (§8.3):
        // a new character, one reset to the default spawn, or one `character move --spawn`
        // put on any ground spawn, whatever its stored instance.
        let at_ground_spawn = self
            .content
            .spawns
            .iter()
            .any(|s| s.ground && s.location == pose.location);
        let eid = self.new_entity_id();
        let mut entity = Entity {
            id: eid,
            kind: EntityKind::Player,
            name: record.name.clone(),
            instance,
            pose,
            velocity: Vec3::ZERO,
            motion: Motion::STANDING,
            appearance: appearance.clone(),
            stance: Stance::Relaxed,
            emote: None,
            owner: Some(id),
            block: (0, 0),
            motion_version: 1,
            dirty_props: BTreeSet::new(),
            dirty_appearance: false,
            wander: None,
            removed: false,
        };
        entity.update_block();
        let epoch = 1;
        let enter = WorldEnter {
            player: eid,
            name: record.name.clone(),
            instance,
            epoch,
            server_time: now,
            pose,
            appearance: appearance.clone(),
        };
        self.entities.insert(eid, entity);
        self.players_by_key.insert(key.clone(), eid);
        let grace = at_ground_spawn;
        let grace_us = micros(Duration::from_secs_f64(self.config.movement.settle_grace_s));
        let limits = if grace {
            self.grace_limits
        } else {
            self.limits
        };
        if let Some(c) = self.conns.get_mut(&id) {
            c.entity = Some(eid);
            c.character_key = Some(key);
            c.validator = Some(MovementValidator::new(limits, epoch, pose, now));
            c.grace_until = if grace { now + grace_us } else { 0 };
            c.last_activity = now;
            c.active_pos = pose.location;
            c.saved_pos = pose.location;
            c.known.clear();
        }
        let first = match self.instance_mut(instance) {
            Some(inst) => {
                inst.players += 1;
                std::mem::take(&mut inst.first_player_groups)
            }
            None => Vec::new(),
        };
        for gi in first {
            self.create_wanderers(instance, gi, &appearance, now);
        }
        self.cmd(id, SessionCmd::EnterWorld(Box::new(enter)));
        let clock = self
            .instance_ref(instance)
            .map(|i| (i.game_clock(now), i.rate));
        if let Some((g, rate)) = clock {
            self.send_world(
                id,
                &Message::WorldClock(WorldClock {
                    server_time: now,
                    game_clock: g,
                    rate,
                }),
            );
        }
        self.send_world(
            id,
            &Message::EntityUpdate(EntityUpdate {
                id: eid,
                changes: self.movement_props(),
            }),
        );
        // Store the instance with the character.
        if let Some(f) = self.characters.get_mut(&account)
            && let Some(rec) = f.characters.iter_mut().find(|c| c.name == record.name)
        {
            rec.instance = instance;
        }
        self.save_account(&account, None, false);
        tracing::info!(account = %account, character = %record.name, instance, "entered the world");
    }

    fn instance_mut(&mut self, n: u32) -> Option<&mut Instance> {
        self.instances.iter_mut().find(|i| i.number == n)
    }

    fn instance_ref(&self, n: u32) -> Option<&Instance> {
        self.instances.iter().find(|i| i.number == n)
    }

    fn create_wanderers(&mut self, instance: u32, group: usize, appearance: &Appearance, now: u64) {
        let Some(g) = self.content.wanderers.get(group).cloned() else {
            return;
        };
        let Some(spawn) = self.content.spawns.get(g.spawn).cloned() else {
            return;
        };
        let walk = f64::from(self.config.movement.walk_speed);
        let run = f64::from(self.config.movement.run_speed);
        let gait = if g.speed <= (walk + run) / 2.0 {
            Gait::Walking
        } else {
            Gait::Running
        };
        let n = g.count.max(1);
        for i in 0..n {
            let a = std::f64::consts::TAU * f64::from(i) / f64::from(n);
            let Some(inst) = self.instance_mut(instance) else {
                return;
            };
            let number = inst.next_wanderer;
            inst.next_wanderer += 1;
            let wander = Wander {
                center_x: spawn.location.x + 2.0 * g.radius * a.cos(),
                center_y: spawn.location.y + 2.0 * g.radius * a.sin(),
                z: spawn.location.z,
                radius: g.radius,
                speed: g.speed,
                start_angle: a,
                t0: now,
            };
            let (x, y, v, h) = wander.state(now);
            let id = self.new_entity_id();
            let mut name = g.name.replace("{n}", &number.to_string());
            name.truncate(floor(&name, tessera_protocol::limits::MAX_NAME_LEN));
            let mut e = Entity {
                id,
                kind: EntityKind::NonPlayer,
                name,
                instance,
                pose: Pose {
                    location: Location::new(spawn.location.region, x, y, wander.z),
                    heading: h,
                    anchor: spawn.anchor,
                },
                velocity: v,
                motion: Motion::new(gait, false),
                appearance: appearance.clone(),
                stance: Stance::Relaxed,
                emote: None,
                owner: None,
                block: (0, 0),
                motion_version: 1,
                dirty_props: BTreeSet::new(),
                dirty_appearance: false,
                wander: Some(wander),
                removed: false,
            };
            if e.pose.anchor.is_some_and(|an| !an.owns(&e.pose.location)) {
                e.pose.anchor = None;
            }
            e.update_block();
            self.entities.insert(id, e);
        }
    }

    // ------------------------------------------------------------------ leaving

    /// Removes the connection's character from the world and saves it, waiting for room in
    /// the writer queue (§12.2: never skipped). Returns whether the save was queued, or
    /// `None` when no character was in the world.
    fn leave_world(&mut self, id: ConnId) -> Option<bool> {
        let c = self.conns.get_mut(&id)?;
        let eid = c.entity.take()?;
        let key = c.character_key.take();
        c.known.clear();
        c.validator = None;
        c.teleport = None;
        c.save_at = None;
        let account = c.account.clone();
        let mut instance = None;
        if let Some(e) = self.entities.get_mut(&eid) {
            e.removed = true;
            instance = Some(e.instance);
        }
        if let Some(k) = &key
            && self.players_by_key.get(k) == Some(&eid)
        {
            self.players_by_key.remove(k);
        }
        if let Some(inst) = instance.and_then(|n| self.instance_mut(n)) {
            inst.players = inst.players.saturating_sub(1);
        }
        let (Some(account), Some(_)) = (account, key) else {
            return Some(false);
        };
        self.sync_record(&account, eid);
        let saved = self.save_account(&account, None, true);
        tracing::info!(account = %account, "left the world");
        Some(saved)
    }

    /// Copies a player entity's state into its character record.
    fn sync_record(&mut self, account: &str, eid: u64) {
        let Some(e) = self.entities.get(&eid) else {
            return;
        };
        let (name, pose, instance, appearance) =
            (e.name.clone(), e.pose, e.instance, e.appearance.clone());
        if let Some(f) = self.characters.get_mut(account)
            && let Some(rec) = f.characters.iter_mut().find(|c| c.name == name)
        {
            rec.position = Some(StoredPosition {
                location: pose.location,
                heading: pose.heading,
                anchor: pose.anchor,
            });
            rec.instance = instance;
            rec.appearance = StoredAppearance::Valid(appearance);
        }
    }

    // ------------------------------------------------------------------ requests in world

    fn player(&self, id: ConnId) -> Option<(u64, &Entity)> {
        let eid = self.conns.get(&id)?.entity?;
        Some((eid, self.entities.get(&eid)?))
    }

    fn refuse_teleport(&self, id: ConnId, request_id: u32, code: TeleportRefusedCode) {
        tracing::debug!(conn = id, request_id, code = %code, "teleport refused");
        self.send_world(
            id,
            &Message::TeleportRefused(TeleportRefused {
                request_id,
                code,
                text: String::new(),
            }),
        );
    }

    fn on_teleport(&mut self, id: ConnId, t: TeleportRequest, invalid: bool, now: u64) {
        let Some(c) = self.conns.get_mut(&id) else {
            return;
        };
        if c.entity.is_none() {
            return;
        }
        if let Some(old) = c.teleport.take() {
            self.refuse_teleport(id, old.request_id, TeleportRefusedCode::Cancelled);
        }
        if invalid {
            return;
        }
        match self.config.world.teleport {
            TeleportPolicy::Off => {
                self.refuse_teleport(id, t.request_id, TeleportRefusedCode::NotAllowed);
                return;
            }
            TeleportPolicy::Named if !matches!(t.destination, Destination::Named { .. }) => {
                self.refuse_teleport(id, t.request_id, TeleportRefusedCode::NotAllowed);
                return;
            }
            _ => {}
        }
        if let Some(c) = self.conns.get_mut(&id) {
            c.teleport = Some(PendingTeleport {
                request_id: t.request_id,
                destination: t.destination,
                received: now,
            });
        }
    }

    /// Resolves a destination to (pose, settling grace) or a refusal code.
    fn resolve_destination(
        &self,
        id: ConnId,
        d: &Destination,
    ) -> Result<(Pose, bool, u32), TeleportRefusedCode> {
        let (_, me) = self.player(id).ok_or(TeleportRefusedCode::ServerError)?;
        let in_extent = |l: &Location| {
            let ok = |c: f64| (self.limits.extent_min..=self.limits.extent_max).contains(&c);
            ok(l.x) && ok(l.y)
        };
        match d {
            Destination::Location { location, heading }
            | Destination::Ground { location, heading } => {
                if !in_extent(location) {
                    return Err(TeleportRefusedCode::InvalidDestination);
                }
                let ground = matches!(d, Destination::Ground { .. });
                Ok((
                    Pose {
                        location: *location,
                        heading: *heading,
                        anchor: None,
                    },
                    ground,
                    me.instance,
                ))
            }
            Destination::Anchored {
                location,
                heading,
                anchor,
            } => {
                if !in_extent(location) {
                    return Err(TeleportRefusedCode::InvalidDestination);
                }
                Ok((
                    Pose {
                        location: *location,
                        heading: *heading,
                        anchor: Some(*anchor),
                    },
                    false,
                    me.instance,
                ))
            }
            Destination::Named { name } => {
                let name = name.trim();
                if let Some(target) = name.strip_prefix('@') {
                    let key = names::name_key(target.trim());
                    let Some(&eid) = self.players_by_key.get(&key) else {
                        return Err(TeleportRefusedCode::UnknownPlace);
                    };
                    let target = self
                        .entities
                        .get(&eid)
                        .ok_or(TeleportRefusedCode::UnknownPlace)?;
                    if !self.config.world.teleport_to_players || target.instance != me.instance {
                        return Err(TeleportRefusedCode::NotAllowed);
                    }
                    Ok((target.pose, false, me.instance))
                } else {
                    let spawn = self
                        .content
                        .spawn_named(name)
                        .ok_or(TeleportRefusedCode::UnknownPlace)?;
                    Ok((spawn_pose(spawn), spawn.ground, me.instance))
                }
            }
        }
    }

    fn execute_teleport(&mut self, id: ConnId, t: PendingTeleport, now: u64) {
        let (pose, ground, _instance) = match self.resolve_destination(id, &t.destination) {
            Ok(r) => r,
            Err(code) => {
                self.refuse_teleport(id, t.request_id, code);
                return;
            }
        };
        let Some((eid, me)) = self.player(id) else {
            return;
        };
        let old_region = me.region();
        let old_block = me.block;
        let r = u32::from(self.config.world.interest_radius);
        let new_block = interest_block(&pose.location, pose.anchor.as_ref());
        let reset =
            pose.location.region != old_region || block_distance(old_block, new_block) > r + 1;
        let grace_us = micros(Duration::from_secs_f64(self.config.movement.settle_grace_s));
        let (epoch, limits) = {
            let Some(c) = self.conns.get_mut(&id) else {
                return;
            };
            let Some(v) = c.validator.as_mut() else {
                return;
            };
            let epoch = v.epoch().wrapping_add(1);
            c.grace_until = if ground { now + grace_us } else { 0 };
            c.last_teleport = Some(now);
            c.save_at = Some(now + TELEPORT_SAVE_DELAY);
            c.active_pos = pose.location;
            (
                epoch,
                if ground {
                    self.grace_limits
                } else {
                    self.limits
                },
            )
        };
        if reset {
            if let Some(c) = self.conns.get_mut(&id) {
                c.known.clear();
            }
            self.send_world(id, &Message::InterestReset);
        }
        if let Some(c) = self.conns.get_mut(&id)
            && let Some(v) = c.validator.as_mut()
        {
            v.set_limits(limits);
            v.start_epoch(epoch, pose, now);
        }
        if let Some(e) = self.entities.get_mut(&eid) {
            e.pose = pose;
            e.velocity = Vec3::ZERO;
            e.motion = Motion::STANDING;
            e.motion_version += 1;
            e.update_block();
        }
        self.send_world(
            id,
            &Message::LocationReport(LocationReport {
                cause: LocationCause::Teleported,
                request_id: t.request_id,
                epoch,
                server_time: now,
                pose,
            }),
        );
    }

    fn on_query_location(&mut self, id: ConnId, request_id: u32, now: u64) {
        let Some((_, me)) = self.player(id) else {
            return;
        };
        let Some(epoch) = self
            .conns
            .get(&id)
            .and_then(|c| c.validator.as_ref())
            .map(MovementValidator::epoch)
        else {
            return;
        };
        let pose = me.pose;
        let instance = me.instance;
        let report = Message::LocationReport(LocationReport {
            cause: LocationCause::Requested,
            request_id,
            epoch,
            server_time: now,
            pose,
        });
        self.send_world(id, &report);
        // The server's own description, numbers only (§8.5).
        let (bx, by) = interest_block(&pose.location, pose.anchor.as_ref());
        let lx = pose.location.x - 160.0 * f64::from(bx);
        let ly = pose.location.y - 160.0 * f64::from(by);
        let mut text = format!(
            "Instance {instance}, region {}, landblock ({bx}, {by}), x {lx:.1}, y {ly:.1}, z {:.1}, facing {}°",
            pose.location.region,
            pose.location.z,
            compass_from_heading(pose.heading)
        );
        let interior =
            pose.anchor.is_some() && !((0.0..=160.0).contains(&lx) && (0.0..=160.0).contains(&ly));
        if interior {
            text.push_str(", interior");
        }
        if let Some(s) = self.content.spawns.iter().find(|s| {
            s.location.region == pose.location.region
                && s.location.distance_xy(&pose.location) <= NEARBY_SPAWN
        }) {
            text.push_str(&format!(", near spawn \"{}\"", s.name));
        }
        self.system_line(id, &text, now);
    }

    fn reject_chat(&mut self, id: ConnId, code: ChatRejectedCode, now: u64) {
        self.cmd(id, SessionCmd::RejectChat(code));
        let flood = self
            .conns
            .get_mut(&id)
            .is_some_and(|c| c.chat_rejections.record(now) >= c.flood_limit);
        if flood {
            self.disconnect(id, AppErrorCode::Flooding, "");
        }
    }

    fn on_chat(&mut self, id: ConnId, c: ChatSend, now: u64) {
        let Some((eid, me)) = self.player(id) else {
            self.reject_chat(id, ChatRejectedCode::NotInWorld, now);
            return;
        };
        let speaker_name = me.name.clone();
        let speaker_loc = me.pose.location;
        let speaker_instance = me.instance;
        let account = self.conns.get(&id).and_then(|c| c.account.clone());
        if account
            .as_deref()
            .and_then(|a| self.accounts.get(a))
            .is_some_and(|a| a.muted)
        {
            self.reject_chat(id, ChatRejectedCode::Muted, now);
            return;
        }
        let allowed = self
            .conns
            .get_mut(&id)
            .is_some_and(|c| c.chat_bucket.take(now));
        if !allowed {
            self.reject_chat(id, ChatRejectedCode::RateLimited, now);
            return;
        }
        let cfg = &self.config.chat;
        let mut recipients: Vec<ConnId> = Vec::new();
        let mut tell_peer = None;
        match &c.channel {
            ChatChannel::Say | ChatChannel::Emote => {
                let range = if matches!(c.channel, ChatChannel::Say) {
                    cfg.say_range_m
                } else {
                    cfg.emote_range_m
                };
                for e in self.entities.values() {
                    if let Some(owner) = e.owner
                        && owner != id
                        && !e.removed
                        && e.instance == speaker_instance
                        && e.region() == speaker_loc.region
                        && e.pose.location.distance_xy(&speaker_loc) <= range
                    {
                        recipients.push(owner);
                    }
                }
            }
            ChatChannel::Yell => {
                for e in self.entities.values() {
                    if let Some(owner) = e.owner
                        && owner != id
                        && !e.removed
                        && e.instance == speaker_instance
                        && (cfg.yell_scope == YellScope::Instance
                            || e.region() == speaker_loc.region)
                    {
                        recipients.push(owner);
                    }
                }
            }
            ChatChannel::Tell { peer } => {
                let key = names::name_key(peer);
                let target = self
                    .players_by_key
                    .get(&key)
                    .and_then(|eid| self.entities.get(eid))
                    .and_then(|e| e.owner.map(|o| (o, e.name.clone())));
                match target {
                    Some((owner, name)) => {
                        if owner != id {
                            recipients.push(owner);
                        }
                        tell_peer = Some(name);
                    }
                    None => {
                        self.reject_chat(id, ChatRejectedCode::UnknownRecipient, now);
                        return;
                    }
                }
            }
            ChatChannel::System => {
                self.cmd(id, SessionCmd::ChatDelivered);
                return;
            }
        }
        if self.config.chat.log_chat {
            tracing::info!(
                target: "tessera_server::chat",
                channel = ?c.channel,
                sender = %speaker_name,
                recipient = tell_peer.as_deref().unwrap_or(""),
                text = %c.text,
                "chat"
            );
        }
        recipients.sort_unstable();
        recipients.dedup();
        let line = |channel: ChatChannel, echo: bool, sender_entity: u64| {
            Message::ChatMessage(ChatMessage {
                server_time: now,
                channel,
                echo,
                sender_entity,
                sender: speaker_name.clone(),
                text: c.text.clone(),
            })
        };
        for r in &recipients {
            let knows = self
                .conns
                .get(r)
                .is_some_and(|rc| rc.known.contains_key(&eid));
            let channel = match &c.channel {
                ChatChannel::Tell { .. } => ChatChannel::Tell {
                    peer: speaker_name.clone(),
                },
                other => other.clone(),
            };
            self.send_chat_frame(*r, &line(channel, false, if knows { eid } else { 0 }));
            self.stats.chat_delivered.fetch_add(1, Ordering::Relaxed);
        }
        let echo_channel = match &c.channel {
            ChatChannel::Tell { .. } => ChatChannel::Tell {
                peer: tell_peer.unwrap_or_default(),
            },
            other => other.clone(),
        };
        self.send_chat_frame(id, &line(echo_channel, true, eid));
        // Answered: session-made rejections of later ChatSends may follow (§6.4.3).
        self.cmd(id, SessionCmd::ChatDelivered);
    }

    fn on_emote(&mut self, id: ConnId, anim: u32, looping: bool, now: u64) {
        let Some(eid) = self.conns.get(&id).and_then(|c| c.entity) else {
            return;
        };
        if let Some(e) = self.entities.get_mut(&eid) {
            e.emote = (anim != 0).then_some(Emote {
                anim,
                started: now,
                looping,
            });
            e.dirty_props.insert(pid::EMOTE);
        }
    }

    fn on_stance(&mut self, id: ConnId, stance: Stance) {
        let Some(eid) = self.conns.get(&id).and_then(|c| c.entity) else {
            return;
        };
        if let Some(e) = self.entities.get_mut(&eid)
            && e.stance != stance
        {
            e.stance = stance;
            e.dirty_props.insert(pid::STANCE);
        }
    }

    fn on_appearance(&mut self, id: ConnId, a: Appearance, now: u64) {
        let Some(eid) = self.conns.get(&id).and_then(|c| c.entity) else {
            return;
        };
        let refused =
            matches!(a.body, Body::Object { .. }) && !self.config.accounts.allow_object_bodies;
        let (result, changed) = {
            let Some(e) = self.entities.get_mut(&eid) else {
                return;
            };
            let changed = !refused && e.appearance != a;
            if changed {
                e.appearance = a;
                e.dirty_appearance = true;
            }
            (e.appearance.clone(), changed)
        };
        self.send_world(
            id,
            &Message::EntityAppearance(EntityAppearance {
                id: eid,
                appearance: result,
            }),
        );
        if refused {
            self.system_line(
                id,
                "Appearance not changed: this server does not accept object bodies for players.",
                now,
            );
        } else if changed {
            // Written at once, not at the next periodic round (§12.2).
            self.save_conn(id);
        }
    }

    // ------------------------------------------------------------------ tick

    /// Runs one tick at server time `now` (§8.1).
    pub fn tick(&mut self, now: u64) {
        if self.stopped {
            return;
        }
        self.tick = tessera_protocol::time::tick_at(now, DEFAULT_TICK_RATE);
        self.check_closed_flags(now);
        self.process_control(now);
        self.process_movement(now);
        self.housekeeping(now);
        self.advance_wanderers(now);
        self.interest_and_emit(now);
        let every = u64::from(DEFAULT_TICK_RATE / self.config.world.snapshot_rate.max(1)).max(1);
        if self.tick.is_multiple_of(every) {
            self.snapshots(now);
        }
        self.flow_control(now);
        self.purge_removed();
        self.publish_stats(now);
    }

    fn check_closed_flags(&mut self, now: u64) {
        let closed: Vec<ConnId> = self
            .conns
            .iter()
            .filter(|(_, c)| {
                c.shared.closed.load(Ordering::Acquire)
                    && !c.shared.close_queued.load(Ordering::Acquire)
            })
            .map(|(id, _)| *id)
            .collect();
        for id in closed {
            self.on_closed(id, now);
        }
    }

    fn process_control(&mut self, now: u64) {
        let mut budget = CONTROL_PER_TICK;
        let n = self.conn_order.len();
        if n == 0 {
            return;
        }
        let start = self.rr_start % n;
        let order: Vec<ConnId> = self.conn_order[start..]
            .iter()
            .chain(self.conn_order[..start].iter())
            .copied()
            .collect();
        let mut progress = true;
        while budget > 0 && progress {
            progress = false;
            for &id in &order {
                if budget == 0 {
                    break;
                }
                let Some(c) = self.conns.get_mut(&id) else {
                    continue;
                };
                let Some(ev) = c.queue.pop_front() else {
                    continue;
                };
                saturating_dec(&c.shared.inbound_pending);
                progress = true;
                budget -= 1;
                if c.dead {
                    continue;
                }
                if c.entity.is_some() {
                    c.last_activity = now;
                }
                self.on_event(id, ev, now);
            }
        }
        self.rr_start = self.rr_start.wrapping_add(1);
    }

    fn on_event(&mut self, id: ConnId, ev: ServerEvent, now: u64) {
        match ev {
            ServerEvent::Greeted(h) => {
                let cap = usize::try_from(self.config.world.max_entities_cap).unwrap_or(1024);
                if let Some(c) = self.conns.get_mut(&id) {
                    c.max_entities = usize::from(h.max_entities).min(cap);
                }
                tracing::debug!(conn = id, client = %sanitise(&h.client_name), "hello");
            }
            ServerEvent::Login {
                credentials,
                register,
            } => self.on_login(id, credentials.account, credentials.password, register, now),
            ServerEvent::CharacterCreate(c) => self.on_create(id, c.name, c.appearance),
            ServerEvent::EnterWorld { name } => self.on_enter(id, name, now),
            ServerEvent::Teleport(t) => self.on_teleport(id, t, false, now),
            ServerEvent::TeleportInvalid(t) => self.on_teleport(id, t, true, now),
            ServerEvent::QueryLocation { request_id } => {
                self.on_query_location(id, request_id, now)
            }
            ServerEvent::Chat(c) => self.on_chat(id, c, now),
            ServerEvent::Emote(e) => self.on_emote(id, e.anim, e.looping, now),
            ServerEvent::Stance(s) => self.on_stance(id, s),
            ServerEvent::Appearance(a) => self.on_appearance(id, a, now),
            ServerEvent::Move(m) => {
                if let Some(c) = self.conns.get(&id) {
                    c.shared.put_move(m);
                }
            }
            _ => {}
        }
    }

    fn process_movement(&mut self, now: u64) {
        let ids: Vec<ConnId> = self
            .conns
            .iter()
            .filter(|(_, c)| c.entity.is_some() && !c.dead)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.move_one(id, now);
        }
    }

    /// Validates and applies a connection's newest `MoveInput`, if any.
    fn move_one(&mut self, id: ConnId, now: u64) {
        let max_up = self.config.movement.max_up_speed as f32;
        let max_down = self.config.movement.max_down_speed as f32;
        {
            let Some(c) = self.conns.get_mut(&id) else {
                return;
            };
            let Some(m) = c.shared.take_move() else {
                return;
            };
            let Some(eid) = c.entity else { return };
            let grace = now < c.grace_until;
            let Some(v) = c.validator.as_mut() else {
                return;
            };
            v.set_limits(if grace {
                self.grace_limits
            } else {
                self.limits
            });
            match v.check(&m, now) {
                MoveVerdict::Accepted => {
                    self.stats.moves_accepted.fetch_add(1, Ordering::Relaxed);
                    let pose = *v.pose();
                    let mut vel = v.velocity();
                    vel.z = vel.z.clamp(-max_down, max_up);
                    let motion = v.motion();
                    if pose.location.distance_xy(&c.active_pos) >= ACTIVE_DISTANCE
                        || (pose.location.z - c.active_pos.z).abs() >= ACTIVE_DISTANCE
                    {
                        c.last_activity = now;
                        c.active_pos = pose.location;
                    }
                    if pose.location.distance_xy(&c.saved_pos) >= SAVE_DISTANCE {
                        c.needs_save = true;
                    }
                    if let Some(e) = self.entities.get_mut(&eid) {
                        let changed = e.pose != pose || e.velocity != vel || e.motion != motion;
                        e.pose = pose;
                        e.velocity = vel;
                        e.motion = motion;
                        e.update_block();
                        if changed {
                            e.motion_version += 1;
                        }
                        if motion.gait != Gait::Standing && e.emote.is_some() {
                            e.emote = None;
                            e.dirty_props.insert(pid::EMOTE);
                        }
                    }
                }
                MoveVerdict::Corrected(report) => {
                    self.stats.corrections.fetch_add(1, Ordering::Relaxed);
                    tracing::debug!(conn = id, epoch = report.epoch, "movement corrected");
                    if let Some(e) = self.entities.get_mut(&eid) {
                        if e.velocity != Vec3::ZERO || e.motion != Motion::STANDING {
                            e.motion_version += 1;
                        }
                        e.velocity = Vec3::ZERO;
                        e.motion = Motion::STANDING;
                    }
                    self.send_world(id, &Message::LocationReport(report));
                }
                MoveVerdict::Rejected => {
                    self.stats
                        .rejections_dropped
                        .fetch_add(1, Ordering::Relaxed);
                }
                MoveVerdict::StaleEpoch | MoveVerdict::OldSeq => {}
            }
        }
    }

    fn housekeeping(&mut self, now: u64) {
        // Accounts reloaded: removed or disabled accounts are kicked (§3.7).
        let generation = self.accounts.generation();
        if generation != self.accounts_generation {
            self.accounts_generation = generation;
            let kicks: Vec<ConnId> = self
                .online
                .iter()
                .filter(|(name, _)| self.accounts.get(name).is_none_or(|a| a.disabled))
                .map(|(_, id)| *id)
                .collect();
            for id in kicks {
                tracing::debug!(conn = id, "account removed or disabled: kicked");
                self.disconnect(id, AppErrorCode::Kicked, "");
            }
        }
        let idle = u64::from(self.config.world.idle_timeout_min) * 60 * SECOND;
        let interval = u64::from(self.config.world.teleport_interval_ms) * 1_000;
        let ids: Vec<ConnId> = self.conns.keys().copied().collect();
        let mut idle_kicks = Vec::new();
        let mut teleports = Vec::new();
        let mut saves = Vec::new();
        for id in ids {
            let Some(c) = self.conns.get_mut(&id) else {
                continue;
            };
            if c.dead || c.entity.is_none() {
                continue;
            }
            if idle > 0 && now.saturating_sub(c.last_activity) >= idle {
                idle_kicks.push(id);
                continue;
            }
            if let Some(t) = &c.teleport {
                let due = c.last_teleport.map_or(0, |l| l + interval);
                if (now >= due || now.saturating_sub(t.received) >= 9 * SECOND)
                    && let Some(t) = c.teleport.take()
                {
                    teleports.push((id, t));
                }
            }
            if c.save_at.is_some_and(|s| now >= s) {
                c.save_at = None;
                saves.push(id);
            }
        }
        for id in idle_kicks {
            self.disconnect(id, AppErrorCode::Idle, "");
        }
        for (id, t) in teleports {
            self.execute_teleport(id, t, now);
        }
        for id in saves {
            self.save_conn(id);
        }
        // Emotes that end on their own (§7.3).
        for e in self.entities.values_mut() {
            if let Some(em) = e.emote
                && !em.looping
                && now.saturating_sub(em.started) >= EMOTE_TIMEOUT
            {
                e.emote = None;
                e.dirty_props.insert(pid::EMOTE);
            }
        }
        // Periodic save (§12.2).
        if now >= self.next_save {
            self.next_save = now + u64::from(self.config.world.save_interval_s) * SECOND;
            self.periodic_save(now);
        }
        // Throttle tables: entries idle for lockout_max_s are removed (§6.2).
        let keep = u64::from(self.config.accounts.lockout_max_s) * SECOND;
        self.lockouts
            .retain(|_, l| now.saturating_sub(l.last) < keep || l.until > now);
        // An address bucket is dropped once it has refilled: a new one is then the same.
        // (Dropping one that is still refilling would hand out a fresh, full bucket.)
        self.addr_buckets.retain(|_, b| !b.full_at(now));
        // Statistics line (§11.1).
        if self.config.log.stats_interval_s > 0 && now >= self.next_stats {
            self.next_stats = now + u64::from(self.config.log.stats_interval_s) * SECOND;
            let s = self.stats.snapshot();
            let per_instance: Vec<u32> = self.instances.iter().map(|i| i.players).collect();
            tracing::info!(
                connections = s.sim_connections,
                logged_in = s.logged_in,
                in_world = s.in_world,
                per_instance = ?per_instance,
                entities = s.entities,
                interest_pairs = s.interest_pairs,
                tick_last_us = s.tick_last_us,
                tick_max_us = s.tick_max_us,
                overruns = s.overruns,
                bytes_in = s.bytes_in,
                bytes_out = s.bytes_out,
                accounted_bytes = s.accounted_bytes,
                "stats"
            );
        }
    }

    fn save_conn(&mut self, id: ConnId) {
        let Some(c) = self.conns.get_mut(&id) else {
            return;
        };
        let (Some(account), Some(eid)) = (c.account.clone(), c.entity) else {
            return;
        };
        c.needs_save = false;
        if let Some(e) = self.entities.get(&eid) {
            c.saved_pos = e.pose.location;
        }
        self.sync_record(&account, eid);
        self.save_account(&account, None, false);
    }

    fn periodic_save(&mut self, now: u64) {
        let ids: Vec<ConnId> = self
            .conns
            .iter()
            .filter(|(_, c)| c.needs_save && c.entity.is_some() && !c.dead)
            .map(|(id, _)| *id)
            .collect();
        let free = self.writer_free();
        if ids.len() + 1 > free {
            tracing::warn!("writer queue nearly full: periodic save round skipped");
            return;
        }
        for id in ids {
            self.save_conn(id);
        }
        let ws = self.world_state(now);
        self.write_file(self.data_dir.world_state_path(), ws.to_toml(), None, false);
    }

    fn writer_free(&self) -> usize {
        crate::workers::WRITER_QUEUE.saturating_sub(self.stats.writer_jobs.load(Ordering::Relaxed))
    }

    fn advance_wanderers(&mut self, now: u64) {
        for e in self.entities.values_mut() {
            let Some(w) = &e.wander else { continue };
            let (x, y, v, h) = w.state(now);
            e.pose.location.x = x;
            e.pose.location.y = y;
            e.pose.heading = h;
            e.velocity = v;
            e.motion_version += 1;
            if e.pose.anchor.is_some_and(|a| !a.owns(&e.pose.location)) {
                e.pose.anchor = None;
            }
            e.update_block();
        }
    }

    fn interest_and_emit(&mut self, now: u64) {
        let radius = self.config.world.interest_radius;
        let r1 = i32::from(radius) + 1;
        // Spatial index of live entities.
        let mut index: HashMap<(u32, u16, u8, u8), Vec<u64>> = HashMap::new();
        for e in self.entities.values() {
            if !e.removed {
                index
                    .entry((e.instance, e.region(), e.block.0, e.block.1))
                    .or_default()
                    .push(e.id);
            }
        }
        // Owner-visible property changes of each player's own entity.
        let mut own_updates: Vec<(ConnId, u64, Vec<Property>)> = Vec::new();
        for e in self.entities.values() {
            if let Some(owner) = e.owner
                && !e.removed
                && !e.dirty_props.is_empty()
            {
                let changes = e
                    .dirty_props
                    .iter()
                    .map(|p| Property::new(*p, e.property_value(*p)))
                    .collect();
                own_updates.push((owner, e.id, changes));
            }
        }
        for (owner, eid, changes) in own_updates {
            self.send_world(
                owner,
                &Message::EntityUpdate(EntityUpdate { id: eid, changes }),
            );
        }
        let observers: Vec<ConnId> = self
            .conns
            .iter()
            .filter(|(_, c)| c.entity.is_some() && !c.dead)
            .map(|(id, _)| *id)
            .collect();
        let bytes_budget =
            usize::try_from(self.config.world.world_bytes_per_tick).unwrap_or(65_536);
        let creates_budget = usize::try_from(self.config.world.creates_per_tick).unwrap_or(16);
        let mut pairs = 0usize;
        for oid in observers {
            let Some(c) = self.conns.get(&oid) else {
                continue;
            };
            let Some(me_id) = c.entity else { continue };
            let Some(me) = self.entities.get(&me_id) else {
                continue;
            };
            let (inst, region, (bx, by)) = (me.instance, me.region(), me.block);
            let my_loc = me.pose.location;
            let paused = c.world_paused;
            let max_entities = c.max_entities.max(1);
            // Candidates within R + 1 blocks.
            let mut near: HashMap<u64, (u32, f64)> = HashMap::new();
            for dx in -r1..=r1 {
                for dy in -r1..=r1 {
                    let x = i32::from(bx) + dx;
                    let y = i32::from(by) + dy;
                    if !(0..=254).contains(&x) || !(0..=254).contains(&y) {
                        continue;
                    }
                    if let Some(list) = index.get(&(inst, region, x as u8, y as u8)) {
                        for &eid in list {
                            if eid == me_id {
                                continue;
                            }
                            let d = block_distance((bx, by), (x as u8, y as u8));
                            let dist = self
                                .entities
                                .get(&eid)
                                .map_or(f64::MAX, |e| e.pose.location.distance_xy(&my_loc));
                            near.insert(eid, (d, dist));
                        }
                    }
                }
            }
            let mut out: Vec<Vec<u8>> = Vec::new();
            let mut used = 0usize;
            let Some(c) = self.conns.get_mut(&oid) else {
                continue;
            };
            // Leaves (never deferred).
            let mut gone = Vec::new();
            for &kid in c.known.keys() {
                let e = self.entities.get(&kid);
                let reason = match e {
                    None => Some(DestroyReason::Removed),
                    Some(e) if e.removed => Some(DestroyReason::Removed),
                    Some(_) => match near.get(&kid) {
                        Some((d, _)) if in_interest(true, *d, radius) => None,
                        _ => Some(DestroyReason::LeftInterest),
                    },
                };
                if let Some(reason) = reason {
                    gone.push((kid, reason));
                }
            }
            for (kid, reason) in gone {
                c.known.remove(&kid);
                if let Some(b) =
                    encode_frame(&Message::EntityDestroy(EntityDestroy { id: kid, reason }))
                {
                    used += b.len();
                    out.push(b);
                }
            }
            // Pending property and appearance updates of known entities.
            for (&kid, k) in c.known.iter_mut() {
                if let Some(e) = self.entities.get(&kid) {
                    k.pending_props.extend(e.dirty_props.iter().copied());
                    k.pending_appearance |= e.dirty_appearance;
                }
            }
            if !paused {
                let mut ids: Vec<u64> = c
                    .known
                    .iter()
                    .filter(|(_, k)| !k.pending_props.is_empty() || k.pending_appearance)
                    .map(|(id, _)| *id)
                    .collect();
                ids.sort_by(|a, b| {
                    let da = near.get(a).map_or((u32::MAX, f64::MAX), |v| *v);
                    let db = near.get(b).map_or((u32::MAX, f64::MAX), |v| *v);
                    da.0.cmp(&db.0).then(da.1.total_cmp(&db.1))
                });
                for kid in ids {
                    if used >= bytes_budget {
                        break;
                    }
                    let Some(e) = self.entities.get(&kid) else {
                        continue;
                    };
                    let Some(k) = c.known.get_mut(&kid) else {
                        continue;
                    };
                    if !k.pending_props.is_empty() {
                        let changes: Vec<Property> = k
                            .pending_props
                            .iter()
                            .map(|p| Property::new(*p, e.property_value(*p)))
                            .collect();
                        k.pending_props.clear();
                        let m = Message::EntityUpdate(EntityUpdate { id: kid, changes });
                        if let Some(b) = encode_frame(&m) {
                            used += b.len();
                            out.push(b);
                        }
                    }
                    if k.pending_appearance {
                        k.pending_appearance = false;
                        let m = Message::EntityAppearance(EntityAppearance {
                            id: kid,
                            appearance: e.appearance.clone(),
                        });
                        if let Some(b) = encode_frame(&m) {
                            used += b.len();
                            out.push(b);
                        }
                    }
                }
                // Creates, nearest first, within the budgets.
                let mut cands: Vec<(u64, u32, f64)> = near
                    .iter()
                    .filter(|(id, (d, _))| *d <= u32::from(radius) && !c.known.contains_key(id))
                    .map(|(id, (d, dist))| (*id, *d, *dist))
                    .collect();
                cands.sort_by(|a, b| a.1.cmp(&b.1).then(a.2.total_cmp(&b.2)).then(a.0.cmp(&b.0)));
                let mut created = 0usize;
                for (eid, _, _) in cands {
                    if created >= creates_budget
                        || c.known.len() >= max_entities
                        || used >= bytes_budget
                    {
                        break;
                    }
                    let Some(e) = self.entities.get(&eid) else {
                        continue;
                    };
                    let m = Message::EntityCreate(EntityCreate {
                        id: e.id,
                        kind: e.kind,
                        name: e.name.clone(),
                        class: None,
                        server_time: now,
                        pose: e.pose,
                        velocity: e.velocity,
                        motion: e.motion,
                        appearance: e.appearance.clone(),
                        properties: e.properties(),
                    });
                    let Some(b) = encode_frame(&m) else { continue };
                    used += b.len();
                    out.push(b);
                    c.known.insert(
                        eid,
                        Known {
                            sent_motion_version: e.motion_version,
                            last_sent: now,
                            rest_left: 0,
                            pending_props: BTreeSet::new(),
                            pending_appearance: false,
                        },
                    );
                    created += 1;
                }
            }
            pairs += c.known.len();
            for b in out {
                self.send_world_bytes(oid, b);
            }
        }
        for e in self.entities.values_mut() {
            e.dirty_props.clear();
            e.dirty_appearance = false;
        }
        self.stats.interest_pairs.store(pairs, Ordering::Relaxed);
    }

    fn snapshots(&mut self, now: u64) {
        let observers: Vec<ConnId> = self
            .conns
            .iter()
            .filter(|(_, c)| c.entity.is_some() && !c.dead && !c.known.is_empty())
            .map(|(id, _)| *id)
            .collect();
        let max_entries = MAX_MOTION_DATAGRAMS_PER_SNAPSHOT * MAX_MOTION_ENTRIES;
        for oid in observers {
            let Some(c) = self.conns.get_mut(&oid) else {
                continue;
            };
            let Some(me) = c.entity.and_then(|id| self.entities.get(&id)) else {
                continue;
            };
            let my_loc = me.pose.location;
            let my_block = me.block;
            let mut due: Vec<(bool, u32, f64, u64)> = Vec::new();
            for (&kid, k) in c.known.iter_mut() {
                let Some(e) = self.entities.get(&kid) else {
                    continue;
                };
                let changed = e.motion_version != k.sent_motion_version;
                let overdue = now.saturating_sub(k.last_sent) >= MOTION_REFRESH;
                let include = changed || k.rest_left > 0 || overdue;
                if include {
                    due.push((
                        overdue,
                        block_distance(my_block, e.block),
                        e.pose.location.distance_xy(&my_loc),
                        kid,
                    ));
                }
            }
            if due.is_empty() {
                continue;
            }
            due.sort_by(|a, b| {
                b.0.cmp(&a.0)
                    .then(a.1.cmp(&b.1))
                    .then(a.2.total_cmp(&b.2))
                    .then(a.3.cmp(&b.3))
            });
            due.truncate(max_entries);
            let mut entries = Vec::with_capacity(due.len());
            for (_, _, _, kid) in due {
                let Some(e) = self.entities.get(&kid) else {
                    continue;
                };
                let Some(k) = c.known.get_mut(&kid) else {
                    continue;
                };
                if e.motion_version != k.sent_motion_version {
                    k.rest_left = if e.at_rest() { REST_SNAPSHOTS - 1 } else { 0 };
                } else {
                    k.rest_left = k.rest_left.saturating_sub(1);
                }
                k.sent_motion_version = e.motion_version;
                k.last_sent = now;
                let l = e.pose.location;
                entries.push(MotionEntry {
                    id: kid,
                    position: Vec3::new(l.x as f32, l.y as f32, l.z as f32),
                    heading: e.pose.heading,
                    velocity: e.velocity,
                    motion: e.motion,
                });
            }
            let shared = Arc::clone(&c.shared);
            for chunk in entries.chunks(MAX_MOTION_ENTRIES) {
                let msg = Message::MotionBatch(MotionBatch {
                    server_time: now,
                    entries: chunk.to_vec(),
                });
                if let Ok(bytes) = encode_datagram(&msg) {
                    shared.send(Outbound::Datagram(bytes));
                }
            }
        }
    }

    fn flow_control(&mut self, now: u64) {
        let mut slow = Vec::new();
        let mut accounted = 0usize;
        let mut backlogs: Vec<(usize, ConnId)> = Vec::new();
        let mut dying = 0usize;
        for (&id, c) in self.conns.iter_mut() {
            let world = c.shared.world_unsent.load(Ordering::Relaxed);
            let chat = c.shared.chat_unsent.load(Ordering::Relaxed);
            let unsent = c.shared.unsent();
            let inbound = c.shared.inbound_pending.load(Ordering::Relaxed);
            accounted += unsent + inbound * INBOUND_EVENT_BYTES + CONNECTION_STATE_BYTES;
            if c.dead {
                // Already ending: its backlog is being freed.
                dying += unsent;
                continue;
            }
            backlogs.push((unsent, id));
            if world >= WORLD_BACKLOG_FATAL {
                slow.push(id);
                continue;
            }
            if world >= WORLD_BACKLOG_PAUSE {
                c.world_paused = true;
                let since = *c.world_over_since.get_or_insert(now);
                if now.saturating_sub(since) >= micros(WORLD_BACKLOG_GRACE) {
                    slow.push(id);
                    continue;
                }
            } else {
                c.world_over_since = None;
                if c.world_paused && world < WORLD_BACKLOG_RESUME {
                    c.world_paused = false;
                }
            }
            if chat > CHAT_BACKLOG_HARD {
                slow.push(id);
                continue;
            }
            if chat > CHAT_BACKLOG_SOFT {
                let since = *c.chat_over_since.get_or_insert(now);
                if now.saturating_sub(since) >= CHAT_BACKLOG_GRACE {
                    slow.push(id);
                }
            } else {
                c.chat_over_since = None;
            }
        }
        for id in slow {
            self.disconnect(id, AppErrorCode::SlowConsumer, "");
        }
        // Connections past the handshake that the simulation does not know (refused by
        // admission and waiting for their Hello answer, or just starting) cost their
        // fixed state too.
        let outside = self
            .stats
            .connections
            .load(Ordering::Relaxed)
            .saturating_sub(self.conns.len());
        accounted += outside * CONNECTION_STATE_BYTES;
        let a = &self.config.accounts;
        accounted +=
            usize::try_from(u64::from(a.argon2_memory_kib) * 1024 * u64::from(a.hash_workers))
                .unwrap_or(usize::MAX / 2);
        let budget = usize::try_from(u64::from(self.config.limits.memory_budget_mib) * 1024 * 1024)
            .unwrap_or(usize::MAX);
        // Backlogs of connections that are already ending do not trigger more disconnects.
        let mut effective = accounted.saturating_sub(dying);
        if effective >= budget {
            // Disconnect the largest backlogs until below 90 % (§10.2).
            backlogs.sort_unstable_by(|a, b| b.cmp(a));
            let target = budget / 10 * 9;
            for (unsent, id) in backlogs {
                if effective < target {
                    break;
                }
                // A connection disconnected earlier in this tick frees its backlog too.
                if self.conns.get(&id).is_some_and(|c| !c.dead) {
                    self.disconnect(id, AppErrorCode::SlowConsumer, "");
                }
                effective = effective.saturating_sub(unsent);
            }
        }
        self.stats
            .accounted_bytes
            .store(accounted, Ordering::Relaxed);
    }

    fn purge_removed(&mut self) {
        self.entities.retain(|_, e| !e.removed);
    }

    fn publish_stats(&self, _now: u64) {
        let s = &self.stats;
        s.sim_connections.store(self.conns.len(), Ordering::Relaxed);
        s.logged_in.store(self.online.len(), Ordering::Relaxed);
        s.in_world
            .store(self.players_by_key.len(), Ordering::Relaxed);
        s.entities.store(self.entities.len(), Ordering::Relaxed);
        s.throttle_entries.store(
            self.lockouts.len() + self.addr_buckets.len(),
            Ordering::Relaxed,
        );
        s.character_files
            .store(self.characters.len(), Ordering::Relaxed);
        let mut queued = 0;
        let mut bytes = 0;
        for c in self.conns.values() {
            queued += c.shared.inbound_pending.load(Ordering::Relaxed);
            bytes += c.shared.unsent();
        }
        s.inbound_queued.store(queued, Ordering::Relaxed);
        s.queue_bytes.store(bytes, Ordering::Relaxed);
    }

    // ------------------------------------------------------------------ shutdown

    fn shutdown_notice(&self, seconds: u16) {
        let text = if seconds == 0 {
            "The server is shutting down now.".to_owned()
        } else {
            format!("The server is shutting down in {seconds} s.")
        };
        let msg = Message::ServerNotice(ServerNotice {
            level: NoticeLevel::Shutdown,
            seconds,
            text,
        });
        for (&id, c) in &self.conns {
            if c.account.is_some() && !c.dead {
                self.send_chat_frame(id, &msg);
            }
        }
    }

    fn stop(&mut self, now: u64) -> StopReport {
        let ids: Vec<ConnId> = self.conns.keys().copied().collect();
        let mut saved = 0;
        let mut not_queued = 0;
        for id in &ids {
            // Shutdown saves are never skipped (§10.1): leave_world waits for room.
            match self.leave_world(*id) {
                Some(true) => saved += 1,
                Some(false) => not_queued += 1,
                None => {}
            }
        }
        let ws = self.world_state(now);
        if !self.write_file(self.data_dir.world_state_path(), ws.to_toml(), None, true) {
            not_queued += 1;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let failures = not_queued
            + if self.writer.send(WriterMsg::Barrier(tx)).is_ok() {
                rx.recv_timeout(Duration::from_secs(60)).unwrap_or(1)
            } else {
                1
            };
        for id in ids {
            self.release_account(id);
            if let Some(c) = self.conns.get_mut(&id) {
                c.dead = true;
                if !c.shared.send(Outbound::Cmd(SessionCmd::Disconnect(
                    AppErrorCode::ServerShutdown,
                    String::new(),
                ))) {
                    c.shared.kill(AppErrorCode::ServerShutdown);
                }
            }
        }
        self.purge_removed();
        self.stopped = true;
        self.publish_stats(now);
        StopReport { saved, failures }
    }
}

fn saturating_dec(a: &std::sync::atomic::AtomicUsize) {
    let mut cur = a.load(Ordering::Acquire);
    while cur > 0 {
        match a.compare_exchange_weak(cur, cur - 1, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return,
            Err(v) => cur = v,
        }
    }
}

fn spawn_pose(s: &Spawn) -> Pose {
    Pose {
        location: s.location,
        heading: s.heading,
        anchor: s.anchor,
    }
}

fn floor(s: &str, max: usize) -> usize {
    let mut end = s.len().min(max);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    end
}

/// A message as a stream frame; `None` (logged) if it cannot be encoded.
fn encode_frame(m: &Message) -> Option<Vec<u8>> {
    match frame_to_vec(m) {
        Ok(b) => Some(b),
        Err(e) => {
            tracing::error!(error = %e, ty = %m.message_type(), "message not encodable");
            None
        }
    }
}

/// Runs the simulation thread until it is stopped or its input channel closes (§8.1):
/// inputs are handled as they arrive, and ticks run every 1/30 s of server time. When more
/// than 5 ticks behind, it skips to the current tick and counts the skipped ones.
pub fn run(mut sim: Sim, rx: &std::sync::mpsc::Receiver<SimInput>, clock: crate::shared::Clock) {
    let tick_us = SECOND / u64::from(DEFAULT_TICK_RATE);
    // Each deadline is tick_start(k) of tessera-protocol.md §11.1, never the previous one
    // plus a rounded period (33,333 µs at 30 Hz would drift 10 µs a second from the
    // schedule and repeat tick numbers).
    let mut k = tessera_protocol::time::tick_at(clock.now(), DEFAULT_TICK_RATE) + 1;
    let mut next = tessera_protocol::time::tick_start(k, DEFAULT_TICK_RATE);
    loop {
        // Wait for the next tick, handling inputs as they come.
        loop {
            let now = clock.now();
            if now >= next {
                break;
            }
            match rx.recv_timeout(Duration::from_micros(next - now)) {
                Ok(input) => sim.handle(input, clock.now()),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
            }
            if sim.stopped() {
                return;
            }
        }
        while let Ok(input) = rx.try_recv() {
            sim.handle(input, clock.now());
            if sim.stopped() {
                return;
            }
        }
        let start = clock.now();
        sim.tick(start);
        let end = clock.now();
        let took = end.saturating_sub(start);
        sim.stats.ticks.fetch_add(1, Ordering::Relaxed);
        sim.stats.tick_last_us.store(took, Ordering::Relaxed);
        sim.stats.tick_max_us.fetch_max(took, Ordering::Relaxed);
        k += 1;
        next = tessera_protocol::time::tick_start(k, DEFAULT_TICK_RATE);
        if end > next {
            sim.stats.overruns.fetch_add(1, Ordering::Relaxed);
            if end.saturating_sub(sim.last_overrun_log) >= 10 * SECOND {
                sim.last_overrun_log = end;
                tracing::warn!(tick_us = took, "simulation tick overran");
            }
            let behind = (end - next) / tick_us;
            if behind > 5 {
                sim.stats.skipped_ticks.fetch_add(behind, Ordering::Relaxed);
                k += behind;
                next = tessera_protocol::time::tick_start(k, DEFAULT_TICK_RATE);
            }
        }
    }
}
