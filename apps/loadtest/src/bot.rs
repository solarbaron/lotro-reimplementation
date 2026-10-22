// SPDX-License-Identifier: GPL-3.0-or-later
//! One bot: a headless player driven through the client's real code path, a
//! [`tessera_net::Session`] over the test-server [`RemoteBackend`] (`test-server.md` §13.1).
//!
//! A bot walks or runs scripted random paths inside an area of landblocks, stops and turns,
//! teleports now and then, changes its appearance, chats when the run asks it to, joins,
//! speaks on and leaves one of the run's shared named channels at protocol minor 2 (the
//! `chat` scenario), and reconnects when told. It records its own true trajectory (what it simulated and sent)
//! and what it sees of everyone else, for the desync checks.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::time::{Duration, Instant};

use tessera_net::remote::{
    AppErrorCode, LoginFailedCode, Password, RemoteBackend, RemoteConfig, RemoteFailure,
    RemoteStats, Stance,
};
use tessera_net::{
    Appearance, Body, ChatChannel, Command, Destination, EntityId, EntityKind, Event, Location,
    LocationCause, RegionId, Session, SocialCommand, SocialEvent, Vec3,
};

use crate::chat::ChatLedger;
use crate::desync::{chebyshev, interest_block};
use crate::metrics::Histogram;
use crate::rng::Rng;
use crate::truth::{Trajectory, TruthSample};

/// The landblock square the bots move in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    /// Region.
    pub region: u16,
    /// West edge, metres (region-global).
    pub min_x: f64,
    /// South edge.
    pub min_y: f64,
    /// East edge.
    pub max_x: f64,
    /// North edge.
    pub max_y: f64,
    /// Height the bots keep.
    pub z: f64,
}

/// Distance kept from the area's edges, metres.
const EDGE_MARGIN: f64 = 1.0;

impl Area {
    /// `blocks` × `blocks` landblocks (1–16) centred on block (`bx`, `by`) of `region`,
    /// at height `z`.
    #[must_use]
    pub fn around(region: u16, bx: u8, by: u8, blocks: u32, z: f64) -> Self {
        let k = i64::from(blocks.clamp(1, 16));
        let lo = |c: u8| (i64::from(c) - (k - 1) / 2).clamp(0, 255 - k);
        let (x0, y0) = (lo(bx) as f64 * 160.0, lo(by) as f64 * 160.0);
        Self {
            region,
            min_x: x0,
            min_y: y0,
            max_x: x0 + k as f64 * 160.0,
            max_y: y0 + k as f64 * 160.0,
            z,
        }
    }

    /// Whether `l` lies in the area (with its margin) and region.
    #[must_use]
    pub fn contains(&self, l: &Location) -> bool {
        l.region.0 == self.region
            && (self.min_x + EDGE_MARGIN..=self.max_x - EDGE_MARGIN).contains(&l.x)
            && (self.min_y + EDGE_MARGIN..=self.max_y - EDGE_MARGIN).contains(&l.y)
    }

    /// A random point inside the margin.
    pub fn random_point(&self, rng: &mut Rng) -> (f64, f64) {
        (
            rng.range(self.min_x + EDGE_MARGIN, self.max_x - EDGE_MARGIN),
            rng.range(self.min_y + EDGE_MARGIN, self.max_y - EDGE_MARGIN),
        )
    }

    /// A random location in the area.
    pub fn random_location(&self, rng: &mut Rng) -> Location {
        let (x, y) = self.random_point(rng);
        Location::new(RegionId(self.region), x, y, self.z)
    }

    /// Side of the area in landblocks.
    #[must_use]
    pub fn blocks(&self) -> u32 {
        ((self.max_x - self.min_x) / 160.0)
            .round()
            .clamp(0.0, 255.0) as u32
    }

    /// Whether some pair of points in the area lies more than `radius` + 1 interest blocks
    /// apart, so the leave side of interest management (`tessera-protocol.md` §9.2) can
    /// happen at all: at least `radius` + 3 blocks.
    #[must_use]
    pub fn allows_leave(&self, radius: u32) -> bool {
        self.blocks() >= radius.saturating_add(3)
    }

    /// Whether every point of the area has a point at least `radius` + 2 blocks away, so
    /// every far teleport can leave interest range: at least 2 × `radius` + 4 blocks.
    #[must_use]
    pub fn allows_far_teleports(&self, radius: u32) -> bool {
        self.blocks() >= radius.saturating_mul(2).saturating_add(4)
    }

    /// A random point whose interest block is at least `min_blocks` blocks (Chebyshev)
    /// from the one of (`x`, `y`); `None` when 64 tries found none (the area is too small
    /// around that place).
    pub fn random_point_beyond(
        &self,
        rng: &mut Rng,
        x: f64,
        y: f64,
        min_blocks: u32,
    ) -> Option<(f64, f64)> {
        let here = interest_block(x, y);
        (0..64)
            .map(|_| self.random_point(rng))
            .find(|p| chebyshev(here, interest_block(p.0, p.1)) >= min_blocks)
    }
}

/// How the bots behave. Intervals are per bot, jittered by ±20 %; `None` turns a behaviour
/// off.
#[derive(Debug, Clone, PartialEq)]
pub struct Behaviour {
    /// Fraction of bots that keep walking; the others idle with long pauses and short walks.
    pub walk_fraction: f64,
    /// Fraction of walking legs at run speed (the rest at walk speed).
    pub run_fraction: f64,
    /// Walk speed, m/s (`test-server.md` §4.5); replaced by the `WALK_SPEED` the server
    /// sends for the player (`tessera-protocol.md` §7.3).
    pub walk_speed: f64,
    /// Run speed, m/s; replaced by the server's `RUN_SPEED`.
    pub run_speed: f64,
    /// One chat line per bot per interval.
    pub chat_interval: Option<Duration>,
    /// One teleport per bot per interval.
    pub teleport_interval: Option<Duration>,
    /// Fraction of teleports that go far (out of interest range of the old place).
    pub far_teleport_fraction: f64,
    /// Every interval, `reconnect_fraction` of the bots disconnect and reconnect.
    pub reconnect_interval: Option<Duration>,
    /// See `reconnect_interval`.
    pub reconnect_fraction: f64,
    /// One appearance change and one stance change per bot per interval.
    pub appearance_interval: Option<Duration>,
    /// Cheating bots (scenario `cheat`): one move far above the speed limit per interval.
    pub cheat_interval: Option<Duration>,
    /// Fraction of bots that cheat.
    pub cheat_fraction: f64,
    /// Shared named channels (protocol minor 2, `test-server.md` §13.1 `chat`): bots whose
    /// session negotiated channels join channel (index / [`CHANNEL_GROUP`]) mod `channels`
    /// and send a share of their lines on it. 0: none.
    pub channels: u8,
    /// A member leaves its channel and joins it again about this often (jittered ±20 %,
    /// at least [`MIN_CHANNEL_CYCLE`] so that the requests stay within the protocol's
    /// request bucket); `None`: members stay.
    pub channel_cycle: Option<Duration>,
    /// Share of a member's chat lines sent on its channel.
    pub channel_fraction: f64,
    /// Test hook: shifts every recorded truth sample this many metres east, so the desync
    /// checks must report Position episodes. Always 0 in real runs.
    pub truth_bias_m: f64,
}

impl Default for Behaviour {
    fn default() -> Self {
        Self {
            walk_fraction: 0.7,
            run_fraction: 0.5,
            walk_speed: 2.0,
            run_speed: 6.0,
            chat_interval: Some(Duration::from_secs(30)),
            teleport_interval: Some(Duration::from_secs(600)),
            far_teleport_fraction: 0.5,
            reconnect_interval: Some(Duration::from_secs(300)),
            reconnect_fraction: 0.1,
            appearance_interval: Some(Duration::from_secs(300)),
            cheat_interval: None,
            cheat_fraction: 0.0,
            channels: 0,
            channel_cycle: None,
            channel_fraction: 0.3,
            truth_bias_m: 0.0,
        }
    }
}

/// Where the bots connect.
#[derive(Debug, Clone)]
pub struct Endpoint {
    /// The connect string, with its pin.
    pub connect: String,
}

/// A bot's identity.
#[derive(Debug, Clone)]
pub struct BotSpec {
    /// Index in the run.
    pub index: usize,
    /// Account name.
    pub account: String,
    /// Password (its `Debug` is redacted).
    pub password: Password,
    /// Character name.
    pub character: String,
}

/// How the run maps a bot's clock to server time for the truth.
#[derive(Debug, Clone, Copy)]
pub enum TimeBase {
    /// In-process: every bot and the server share one monotonic clock.
    Shared {
        /// A moment.
        base: Instant,
        /// The server time at that moment, µs.
        server_at_base: u64,
    },
    /// Remote: each bot's own clock-offset estimate (`tessera-protocol.md` §11.2).
    Estimated,
}

impl TimeBase {
    /// Server time at `now`, µs.
    #[must_use]
    pub fn server_us(&self, now: Instant, backend: Option<&RemoteBackend>) -> Option<u64> {
        match self {
            Self::Shared {
                base,
                server_at_base,
            } => {
                let d = u64::try_from(now.saturating_duration_since(*base).as_micros()).ok()?;
                server_at_base.checked_add(d)
            }
            Self::Estimated => backend?.server_time(now),
        }
    }
}

/// What a bot counted.
#[derive(Debug, Clone, Default)]
pub struct BotStats {
    /// Sessions started.
    pub sessions: u64,
    /// Successful logins.
    pub logins: u64,
    /// World entries.
    pub world_entries: u64,
    /// Planned reconnections done.
    pub reconnects: u64,
    /// Login latency (Connect to Connected), ms.
    pub login_ms: Histogram,
    /// World-entry latency (EnterWorld to WorldEntered), ms.
    pub enter_ms: Histogram,
    /// Application round-trip time (`Ping`), ms: each new measurement once (the backend
    /// pings every 2 s; a value equal to the previous one is taken as the same ping).
    pub rtt_ms: Histogram,
    /// Sessions that ended without the bot asking.
    pub unexpected_disconnects: u64,
    /// Logins the server refused for admission (`RATE_LIMITED`, `SERVER_FULL`); the bot
    /// retried after the time the server asked for. Expected in `connect-storm`.
    pub admission_refusals: u64,
    /// Failure descriptions (the first 16).
    pub failures: Vec<String>,
    /// `CORRECTED` location reports.
    pub corrections: u64,
    /// Teleports answered with a location report.
    pub teleports: u64,
    /// Teleports refused (a `teleport refused:` line while one was pending).
    pub teleports_refused: u64,
    /// `teleport refused:` lines with no teleport pending.
    pub refusals_unmatched: u64,
    /// Teleports with no answer within 10 s.
    pub teleports_unanswered: u64,
    /// Far teleports whose destination lies more than *R* + 1 blocks from the old place.
    pub far_teleports: u64,
    /// Far teleports that could not leave *R* + 1 blocks (the area is too small there).
    pub far_teleports_short: u64,
    /// Chat lines sent.
    pub chat_sent: u64,
    /// Chat lines the server rejected (`chat not sent:` lines).
    pub chat_rejected: u64,
    /// Channel joins answered with `ChannelJoined`.
    pub channel_joins: u64,
    /// Channel leaves answered with `ChannelLeft`.
    pub channel_leaves: u64,
    /// Channel requests refused (`ChannelRefused`; a failure for a bot within the limits).
    pub channel_refusals: u64,
    /// System lines received (all kinds).
    pub system_lines: u64,
    /// Commands the session refused (full queue, wrong state).
    pub send_errors: u64,
    /// Appearance changes requested.
    pub appearance_changes: u64,
    /// Stance changes requested.
    pub stance_changes: u64,
    /// Cheating moves sent.
    pub cheats: u64,
    /// Cheats answered by a `CORRECTED` report back to the place before the cheat.
    pub cheats_corrected: u64,
    /// Cheats answered by a `CORRECTED` report somewhere else.
    pub cheats_bad_correction: u64,
    /// Cheats with no correction within [`CHEAT_ANSWER`] (correct only if the server
    /// dropped them).
    pub cheats_unanswered: u64,
    /// Cheats not judged because the bot left the world first.
    pub cheats_abandoned: u64,
    /// Further corrections back to the same place while a cheat was open.
    pub cheat_extra_corrections: u64,
    /// Sessions given up while closing (no `Disconnected` within 10 s).
    pub close_timeouts: u64,
    /// Transport counters summed over finished sessions (and the current one at the end).
    pub traffic: Traffic,
}

/// Transport counters of the backend (`RemoteStats`), summed over sessions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Traffic {
    /// UDP bytes sent.
    pub udp_sent: u64,
    /// UDP bytes received.
    pub udp_received: u64,
    /// `MoveInput`s sent.
    pub moves_sent: u64,
    /// `MotionBatch`es received.
    pub motion_batches: u64,
    /// Datagrams received.
    pub datagrams_received: u64,
    /// Movement items dropped because the bot did not keep up.
    pub motion_dropped: u64,
    /// QUIC packets lost.
    pub lost_packets: u64,
    /// Messages dropped as stale or unexpected.
    pub protocol_dropped: u64,
}

impl Traffic {
    fn add(&mut self, s: &RemoteStats) {
        self.udp_sent += s.udp_bytes_sent;
        self.udp_received += s.udp_bytes_received;
        self.moves_sent += s.moves_sent;
        self.motion_batches += s.motion_batches;
        self.datagrams_received += s.datagrams_received;
        self.motion_dropped += s.motion_dropped;
        self.lost_packets += s.lost_packets;
        self.protocol_dropped += s.protocol_dropped;
    }

    /// Adds another total.
    pub fn merge(&mut self, o: &Self) {
        self.udp_sent += o.udp_sent;
        self.udp_received += o.udp_received;
        self.moves_sent += o.moves_sent;
        self.motion_batches += o.motion_batches;
        self.datagrams_received += o.datagrams_received;
        self.motion_dropped += o.motion_dropped;
        self.lost_packets += o.lost_packets;
        self.protocol_dropped += o.protocol_dropped;
    }
}

/// An entity a bot has created (what the observer sees).
#[derive(Debug, Clone)]
pub struct Seen {
    /// Its name.
    pub name: String,
    /// Its kind.
    pub kind: EntityKind,
    /// The bot it is, if any.
    pub bot: Option<usize>,
    /// Its latest appearance.
    pub appearance: Appearance,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Plan {
    Leg { x: f64, y: f64, speed: f64 },
    Pause { until: Instant },
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Offline { at: Instant },
    Connecting { since: Instant },
    Entering { since: Instant },
    InWorld,
    Closing { since: Instant, reconnect: bool },
    Done,
}

/// A cheating move observers must never show (`test-server.md` §13.1 `cheat`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CheatWatch {
    /// The cheat's number for this bot (from 1).
    pub id: u32,
    /// Region of the refused target.
    pub region: u16,
    /// Refused target, metres.
    pub x: f64,
    /// Refused target, metres.
    pub y: f64,
    /// Watched until then (hold, answer time, interpolation delay and a margin).
    pub until: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct OpenCheat {
    at: Instant,
    from: Location,
    answered: bool,
}

/// A bot's membership of its shared channel, as far as the bot knows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelState {
    /// Not a member, nothing in flight.
    Out,
    /// `ChannelJoin` sent, not yet answered.
    Joining,
    /// `ChannelJoined` received.
    Member,
    /// `ChannelLeave` sent, not yet answered.
    Leaving,
}

/// Bots per shared channel group: bots 0–3 use channel 0, 4–7 channel 1, and so on, modulo
/// [`Behaviour::channels`], so that even a two-bot run shares a channel.
pub const CHANNEL_GROUP: usize = 4;
/// Shortest [`Behaviour::channel_cycle`]: a cycle costs two requests, and the protocol's
/// request bucket refills one token per 6 s (`tessera-protocol.md` §10.6).
pub const MIN_CHANNEL_CYCLE: Duration = Duration::from_secs(15);
/// A channel request must be answered within this time.
pub const CHANNEL_ANSWER: Duration = Duration::from_secs(10);

/// What a system line is. `tessera-net` turns `TeleportRefused`, `ChatRejected`,
/// `ChannelRefused` and `ServerNotice` all into [`ChatChannel::System`] lines; the backend's prefixes tell them
/// apart (an operator notice could imitate them; `test-server.md` §15.2 asks for typed
/// events).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemLine {
    /// `teleport refused: CODE[: text]`.
    TeleportRefused,
    /// `chat not sent: CODE[: text]`; `benign` for a recipient that went offline or a
    /// sender between places (`UNKNOWN_RECIPIENT`, `NOT_IN_WORLD`).
    ChatRejected {
        /// Expected in a run with reconnections.
        benign: bool,
    },
    /// `Cannot join the channel …` or `Cannot leave the channel …` (`ChannelRefused`).
    ChannelRefused,
    /// Anything else (notices, location descriptions).
    Other,
}

/// Classifies a system line by the backend's prefixes.
#[must_use]
pub fn classify_system_line(text: &str) -> SystemLine {
    if text.starts_with("teleport refused:") {
        return SystemLine::TeleportRefused;
    }
    if let Some(rest) = text.strip_prefix("chat not sent:") {
        let code = rest
            .trim_start()
            .split(|c: char| c == ':' || c.is_whitespace())
            .next()
            .unwrap_or("");
        return SystemLine::ChatRejected {
            benign: matches!(code, "UNKNOWN_RECIPIENT" | "NOT_IN_WORLD"),
        };
    }
    if text.starts_with("Cannot join the channel ") || text.starts_with("Cannot leave the channel ")
    {
        return SystemLine::ChannelRefused;
    }
    SystemLine::Other
}

/// What a bot needs from the run while it handles its events.
#[derive(Debug)]
pub struct Ctx<'a> {
    /// Now.
    pub now: Instant,
    /// Character name → bot index.
    pub names: &'a HashMap<String, usize>,
    /// The chat ledger.
    pub ledger: &'a mut ChatLedger,
    /// The time base for the truth.
    pub time: TimeBase,
    /// The area.
    pub area: &'a Area,
    /// The interest radius *R* in blocks (far teleports go beyond *R* + 1).
    pub radius: u32,
}

/// Longest wait for a login, a world entry or a close.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(60);
const ENTER_TIMEOUT: Duration = Duration::from_secs(30);
const CLOSE_TIMEOUT: Duration = Duration::from_secs(10);
/// A teleport must be answered within this time (`test-server.md` §8.4).
pub const TELEPORT_TIMEOUT: Duration = Duration::from_secs(10);
/// Most failure descriptions kept per bot.
const MAX_FAILURES: usize = 16;
/// Most entities a bot keeps track of (its `max_entities`).
pub const MAX_ENTITIES: usize = 1024;
/// Most history entries (sessions, teleports) kept per bot.
const MAX_HISTORY: usize = 64;
/// A cheating move is held this long, so the backend's newest-move slot sends it.
pub const CHEAT_HOLD: Duration = Duration::from_millis(200);
/// A cheat must be corrected within this time after its hold (or dropped by the server).
pub const CHEAT_ANSWER: Duration = Duration::from_secs(2);
/// A correction of a cheat must put the bot back within this distance of where it was,
/// metres (the server's last accepted position; one tick of running at most).
pub const CHEAT_POSE_TOLERANCE: f64 = 1.5;
/// Observers are watched for the refused target this long after the cheat: hold, answer
/// time, the interpolation delay and a second of margin.
pub const CHEAT_WATCH: Duration = Duration::from_millis(200 + 2_000 + 1_500);
/// Distance of a cheating move, metres (far above the 8.5 m/s × 1 s allowance, §8.3).
pub const CHEAT_DISTANCE: f64 = 40.0;

/// One bot.
#[derive(Debug)]
pub struct Bot {
    /// Identity.
    pub spec: BotSpec,
    endpoint: Endpoint,
    session: Option<Session>,
    phase: Phase,
    events: Vec<Event>,
    rng: Rng,
    /// Walks continuously (otherwise idles with short walks).
    pub walker: bool,
    /// Cheats now and then (scenario `cheat`).
    pub cheater: bool,
    /// The player's entity in the current session.
    pub player: Option<EntityId>,
    /// The position is known (a location report arrived).
    located: bool,
    /// The true position.
    pub pos: Location,
    heading: f32,
    vel: Vec3,
    plan: Plan,
    /// When a pending teleport was requested.
    pub teleport_pending: Option<Instant>,
    /// Recent answered teleports and corrections (when the position jumped).
    pub jumps: VecDeque<Instant>,
    /// In-world intervals: entry and (when left) exit.
    pub online: VecDeque<(Instant, Option<Instant>)>,
    next_chat: Option<Instant>,
    next_teleport: Option<Instant>,
    next_appearance: Option<Instant>,
    next_cheat: Option<Instant>,
    /// A cheating move being held (until, where), so the newest-move slot sends it.
    cheat_hold: Option<(Instant, Location)>,
    open_cheat: Option<OpenCheat>,
    /// The latest cheat, while observers are watched for it.
    pub cheat_watch: Option<CheatWatch>,
    cheat_seq: u32,
    /// Its stance as it last set it (Relaxed at world entry).
    pub stance: Stance,
    next_rtt: Instant,
    last_rtt: Option<Duration>,
    chat_seq: u32,
    /// Membership of the bot's shared channel.
    channel: ChannelState,
    /// When the request in flight was sent.
    channel_since: Instant,
    /// Membership changes: request sent, answer received, world left (bounded).
    channel_history: VecDeque<(Instant, ChannelState)>,
    /// Entries were dropped from `channel_history`.
    channel_history_cut: bool,
    /// When the next join or leave is due (`None`: none planned).
    next_channel: Option<Instant>,
    connect_started: Option<Instant>,
    enter_started: Option<Instant>,
    /// Entities this bot has created, by local ID.
    pub view: BTreeMap<EntityId, Seen>,
    /// Bot index → local ID in [`view`](Self::view).
    pub view_bot: HashMap<usize, EntityId>,
    /// Its own appearance as the server confirmed it.
    pub appearance: Option<Appearance>,
    /// When `appearance` last changed.
    pub appearance_changed: Option<Instant>,
    /// The true trajectory.
    pub trajectory: Trajectory,
    /// Corrections received (time, distance moved back), drained by the run.
    pub corrections: Vec<(Instant, f64)>,
    /// Counters.
    pub stats: BotStats,
    behaviour: Behaviour,
    finishing: bool,
    /// The current session's counters were already added to the totals.
    traffic_flushed: bool,
}

/// The seconds to wait when the server refused a login for admission (rate limit or a
/// full server), `None` for any other end.
fn admission_refusal(f: Option<&RemoteFailure>) -> Option<u16> {
    match f? {
        RemoteFailure::Login { code, retry_after }
            if matches!(
                code.canonical(),
                LoginFailedCode::RateLimited | LoginFailedCode::ServerFull
            ) =>
        {
            Some(*retry_after)
        }
        RemoteFailure::Server { code } if code.canonical() == AppErrorCode::ServerFull => Some(2),
        _ => None,
    }
}

fn speed_of(v: Vec3) -> f64 {
    f64::from(v.x).hypot(f64::from(v.y))
}

fn jitter(rng: &mut Rng, d: Duration) -> Duration {
    d.mul_f64(rng.range(0.8, 1.2))
}

impl Bot {
    /// A bot that connects at `start`.
    #[must_use]
    pub fn new(
        spec: BotSpec,
        endpoint: Endpoint,
        behaviour: Behaviour,
        seed: u64,
        start: Instant,
    ) -> Self {
        let mut rng = Rng::new(seed);
        let walker = rng.chance(behaviour.walk_fraction);
        let cheater = behaviour.cheat_interval.is_some() && rng.chance(behaviour.cheat_fraction);
        Self {
            spec,
            endpoint,
            session: None,
            phase: Phase::Offline { at: start },
            events: Vec::new(),
            rng,
            walker,
            cheater,
            player: None,
            located: false,
            pos: Location::new(RegionId(0), 0.0, 0.0, 0.0),
            heading: 0.0,
            vel: Vec3::ZERO,
            plan: Plan::Pause { until: start },
            teleport_pending: None,
            jumps: VecDeque::new(),
            online: VecDeque::new(),
            next_chat: None,
            next_teleport: None,
            next_appearance: None,
            next_cheat: None,
            cheat_hold: None,
            open_cheat: None,
            cheat_watch: None,
            cheat_seq: 0,
            stance: Stance::Relaxed,
            next_rtt: start,
            last_rtt: None,
            chat_seq: 0,
            channel: ChannelState::Out,
            channel_since: start,
            channel_history: VecDeque::new(),
            channel_history_cut: false,
            next_channel: None,
            connect_started: None,
            enter_started: None,
            view: BTreeMap::new(),
            view_bot: HashMap::new(),
            appearance: None,
            appearance_changed: None,
            trajectory: Trajectory::new(),
            corrections: Vec::new(),
            stats: BotStats::default(),
            behaviour,
            finishing: false,
            traffic_flushed: false,
        }
    }

    /// The backend of the current session.
    #[must_use]
    pub fn backend(&self) -> Option<&RemoteBackend> {
        self.session.as_ref()?.backend::<RemoteBackend>()
    }

    /// In the world with a known position (a teleport may be pending: see
    /// [`teleport_pending`](Self::teleport_pending)).
    #[must_use]
    pub fn in_world(&self) -> bool {
        self.phase == Phase::InWorld && self.located
    }

    /// The session has ended for good (after [`finish`](Self::finish)).
    #[must_use]
    pub fn done(&self) -> bool {
        matches!(self.phase, Phase::Done)
    }

    /// Horizontal speed now, m/s.
    #[must_use]
    pub fn speed(&self) -> f64 {
        speed_of(self.vel)
    }

    /// Walk and run speeds in use, m/s (the server's, once it sent them).
    #[must_use]
    pub fn speeds(&self) -> (f64, f64) {
        (self.behaviour.walk_speed, self.behaviour.run_speed)
    }

    /// A cheating move is being held or awaits its answer: heading and gait are not judged.
    #[must_use]
    pub fn cheating(&self) -> bool {
        self.cheat_hold.is_some() || self.open_cheat.is_some_and(|c| !c.answered)
    }

    /// Whether the bot was in the world for the whole of [`from`, `to`] (a session still
    /// open counts up to now).
    #[must_use]
    pub fn online_during(&self, from: Instant, to: Instant) -> bool {
        self.online
            .iter()
            .any(|(s, e)| *s <= from && e.is_none_or(|e| e >= to))
    }

    /// Whether the bot's position jumped (teleport, correction) or a teleport was pending
    /// during [`from`, `to`].
    #[must_use]
    pub fn jumped_during(&self, from: Instant, to: Instant) -> bool {
        self.jumps.iter().any(|t| *t >= from && *t <= to)
            || self.teleport_pending.is_some_and(|t| t <= to)
    }

    fn fail(&mut self, what: String) {
        tracing::warn!(bot = self.spec.index, %what, "bot failure");
        if self.stats.failures.len() < MAX_FAILURES {
            self.stats.failures.push(what);
        }
    }

    fn send(&mut self, now: Instant, cmd: Command) -> bool {
        let Some(s) = self.session.as_mut() else {
            return false;
        };
        match s.send(now, cmd) {
            Ok(()) => true,
            Err(e) => {
                self.stats.send_errors += 1;
                tracing::debug!(bot = self.spec.index, error = %e, "command refused");
                false
            }
        }
    }

    fn connect(&mut self, now: Instant) {
        let cfg = match RemoteConfig::new(
            &self.endpoint.connect,
            self.spec.account.clone(),
            self.spec.password.clone(),
        ) {
            Ok(mut c) => {
                c.client.client_name = format!("tessera-loadtest {}", env!("CARGO_PKG_VERSION"));
                // A share of bots stays at minor 0 and another offers 1, so observers at
                // every minor meet (test-server.md §13.1).
                c.client.minor_max = crate::raw::bot_minor(self.spec.index);
                c
            }
            Err(e) => {
                self.fail(format!("config: {e}"));
                self.phase = Phase::Done;
                return;
            }
        };
        let backend = match RemoteBackend::new(cfg) {
            Ok(b) => b,
            Err(e) => {
                self.fail(format!("backend: {e}"));
                self.phase = Phase::Done;
                return;
            }
        };
        self.session = Some(Session::new(backend));
        self.traffic_flushed = false;
        self.stats.sessions += 1;
        self.connect_started = Some(now);
        self.phase = Phase::Connecting { since: now };
        if !self.send(now, Command::Connect) {
            self.fail("connect refused".into());
            self.session = None;
            self.phase = Phase::Offline {
                at: now + Duration::from_secs(5),
            };
        }
    }

    /// Asks the session to end and, unless `reconnect`, stay offline.
    pub fn disconnect(&mut self, now: Instant, reconnect: bool) {
        match self.phase {
            Phase::Connecting { .. } | Phase::Entering { .. } | Phase::InWorld => {
                self.leave_world(now);
                self.send(now, Command::Disconnect);
                self.phase = Phase::Closing {
                    since: now,
                    reconnect,
                };
                if reconnect {
                    self.stats.reconnects += 1;
                }
            }
            Phase::Offline { .. } if !reconnect => self.phase = Phase::Done,
            _ => {}
        }
    }

    /// Stops scheduling chat, teleports, appearance changes and cheats (the run's quiet
    /// tail); movement goes on.
    pub fn quiet(&mut self) {
        let b = &mut self.behaviour;
        b.chat_interval = None;
        b.teleport_interval = None;
        b.appearance_interval = None;
        b.cheat_interval = None;
        b.reconnect_interval = None;
    }

    /// Ends the bot's run: disconnects and stays offline.
    pub fn finish(&mut self, now: Instant) {
        self.finishing = true;
        self.disconnect(now, false);
    }

    fn leave_world(&mut self, now: Instant) {
        if let Some((_, end)) = self.online.back_mut()
            && end.is_none()
        {
            *end = Some(now);
        }
        self.located = false;
        self.player = None;
        self.view.clear();
        self.view_bot.clear();
        self.trajectory.clear();
        self.teleport_pending = None;
        self.cheat_hold = None;
        self.cheat_watch = None;
        // Leaving the world ends every membership, silently (test-server.md §9.1).
        if self.channel != ChannelState::Out {
            self.set_channel(now, ChannelState::Out);
        }
        self.next_channel = None;
        if let Some(c) = self.open_cheat.take()
            && !c.answered
        {
            self.stats.cheats_abandoned += 1;
        }
    }

    fn end_session(&mut self, now: Instant) {
        self.flush_traffic();
        self.session = None;
        self.leave_world(now);
    }

    /// Adds the current session's transport counters to the totals (end of run).
    pub fn flush_traffic(&mut self) {
        if self.traffic_flushed {
            return;
        }
        if let Some(b) = self.backend() {
            let s = b.stats().clone();
            self.stats.traffic.add(&s);
            self.traffic_flushed = true;
        }
    }

    fn record_truth(&mut self, now: Instant, time: TimeBase, jump: bool) {
        if !self.located {
            return;
        }
        let Some(t) = time.server_us(now, self.backend()) else {
            return;
        };
        self.trajectory.record(TruthSample {
            t_us: t,
            region: self.pos.region.0,
            x: self.pos.x + self.behaviour.truth_bias_m,
            y: self.pos.y,
            z: self.pos.z,
            heading: self.heading,
            speed: if jump { 0.0 } else { speed_of(self.vel) },
            jump,
            rest_since_us: None,
        });
    }

    /// Requests a teleport to `to`.
    pub fn teleport(&mut self, now: Instant, to: Location) {
        if self.teleport_pending.is_some() || self.phase != Phase::InWorld {
            return;
        }
        let heading = self.heading;
        if self.send(
            now,
            Command::Teleport {
                destination: Destination::Location {
                    location: to,
                    heading,
                },
            },
        ) {
            self.teleport_pending = Some(now);
            self.vel = Vec3::ZERO;
        }
    }

    /// Sends a chat line on `channel`.
    pub fn chat(&mut self, now: Instant, channel: ChatChannel, text: String) -> bool {
        let ok = self.send(now, Command::Chat { channel, text });
        if ok {
            self.stats.chat_sent += 1;
        }
        ok
    }

    /// The next chat sequence number, if a line is due now.
    pub fn chat_due(&mut self, now: Instant) -> Option<u32> {
        let interval = self.behaviour.chat_interval?;
        if !self.in_world() || self.teleport_pending.is_some() {
            return None;
        }
        let next = *self.next_chat.get_or_insert_with(|| {
            now + interval
                .mul_f64(self.rng.unit())
                .max(Duration::from_secs(2))
        });
        if now < next {
            return None;
        }
        // Never faster than the protocol's sustained 1 line per 2 s (§10.2).
        let gap = jitter(&mut self.rng, interval).max(Duration::from_millis(2_500));
        self.next_chat = Some(now + gap);
        let seq = self.chat_seq;
        self.chat_seq = self.chat_seq.wrapping_add(1);
        Some(seq)
    }

    /// The bot's shared channel number, when the run uses channels.
    #[must_use]
    pub fn channel_slot(&self) -> Option<u8> {
        let n = usize::from(self.behaviour.channels);
        (n > 0).then(|| u8::try_from((self.spec.index / CHANNEL_GROUP) % n).unwrap_or(0))
    }

    /// Its channel's number while it is a member (`ChannelJoined` received) in the world.
    #[must_use]
    pub fn channel_member(&self) -> Option<u8> {
        if self.in_world() && self.channel == ChannelState::Member {
            self.channel_slot()
        } else {
            None
        }
    }

    /// Its channel's number while a join or leave of it is in flight.
    #[must_use]
    pub fn channel_pending(&self) -> Option<u8> {
        if self.phase == Phase::InWorld
            && matches!(self.channel, ChannelState::Joining | ChannelState::Leaving)
        {
            self.channel_slot()
        } else {
            None
        }
    }

    /// Share of its chat lines sent on its channel while a member.
    #[must_use]
    pub fn channel_fraction(&self) -> f64 {
        self.behaviour.channel_fraction
    }

    /// Its membership state now.
    #[must_use]
    pub fn channel_state(&self) -> ChannelState {
        self.channel
    }

    /// Whether its membership held still during [`from`, `to`]: no request sent or answered
    /// and no world exit in it, and no request in flight at its start.
    #[must_use]
    pub fn channel_settled_during(&self, from: Instant, to: Instant) -> bool {
        if self
            .channel_history
            .iter()
            .any(|(t, _)| *t >= from && *t <= to)
        {
            return false;
        }
        let before = self
            .channel_history
            .iter()
            .rev()
            .find(|(t, _)| *t < from)
            .map(|(_, s)| *s);
        let state = match before {
            Some(s) => s,
            None if !self.channel_history_cut => ChannelState::Out,
            None => return false,
        };
        matches!(state, ChannelState::Out | ChannelState::Member)
    }

    fn set_channel(&mut self, now: Instant, state: ChannelState) {
        self.channel = state;
        self.channel_history.push_back((now, state));
        while self.channel_history.len() > MAX_HISTORY {
            self.channel_history.pop_front();
            self.channel_history_cut = true;
        }
    }

    /// Joins or leaves the bot's channel when due (`test-server.md` §13.1 `chat`), and fails
    /// a request left unanswered.
    fn channel_step(&mut self, now: Instant, ledger: &ChatLedger) {
        let Some(k) = self.channel_slot() else {
            return;
        };
        match self.channel {
            ChannelState::Joining | ChannelState::Leaving => {
                if self.next_channel.is_none()
                    && now.saturating_duration_since(self.channel_since) > CHANNEL_ANSWER
                {
                    // Reported once; the state stays in flight, so nothing of this
                    // channel is judged for the bot any more in this session.
                    self.fail("channel request unanswered".into());
                    self.next_channel = Some(now + Duration::from_secs(1 << 20));
                }
            }
            ChannelState::Out | ChannelState::Member => {
                // Quiet tail or a run without chat: no new requests.
                if self.behaviour.chat_interval.is_none() {
                    return;
                }
                let Some(at) = self.next_channel else {
                    return;
                };
                if now < at {
                    return;
                }
                if !self.backend().is_some_and(RemoteBackend::has_chat_channels) {
                    // Minor below 2 or channels off: this session never joins.
                    self.next_channel = None;
                    return;
                }
                let name = ledger.channel_name(k);
                let (cmd, state) = if self.channel == ChannelState::Out {
                    (SocialCommand::JoinChannel { name }, ChannelState::Joining)
                } else {
                    (SocialCommand::LeaveChannel { name }, ChannelState::Leaving)
                };
                if self.send(now, Command::Social(cmd)) {
                    self.next_channel = None;
                    self.channel_since = now;
                    self.set_channel(now, state);
                } else {
                    self.next_channel = Some(now + Duration::from_secs(10));
                }
            }
        }
    }

    fn channel_event(&mut self, now: Instant, e: &SocialEvent, ledger: &ChatLedger) {
        let expected = self.channel_slot().map(|k| ledger.channel_name(k));
        match e {
            SocialEvent::ChannelJoined { channel } => {
                if self.channel == ChannelState::Joining && expected.as_ref() == Some(channel) {
                    self.stats.channel_joins += 1;
                    self.set_channel(now, ChannelState::Member);
                    self.next_channel = self
                        .behaviour
                        .channel_cycle
                        .map(|c| now + jitter(&mut self.rng, c.max(MIN_CHANNEL_CYCLE)));
                } else {
                    self.fail(format!(
                        "unexpected ChannelJoined {channel:?} ({:?})",
                        self.channel
                    ));
                }
            }
            SocialEvent::ChannelLeft { channel } => {
                if self.channel == ChannelState::Leaving && expected.as_ref() == Some(channel) {
                    self.stats.channel_leaves += 1;
                    self.set_channel(now, ChannelState::Out);
                    self.next_channel =
                        Some(now + Duration::from_secs_f64(self.rng.range(1.0, 3.0)));
                } else {
                    // tessera-server never removes a member (test-server.md §9.1).
                    self.fail(format!(
                        "unexpected ChannelLeft {channel:?} ({:?})",
                        self.channel
                    ));
                }
            }
            _ => {}
        }
    }

    /// Polls the session and handles its events.
    pub fn poll(&mut self, ctx: &mut Ctx<'_>) {
        let now = ctx.now;
        let mut events = std::mem::take(&mut self.events);
        if let Some(s) = self.session.as_mut() {
            s.poll_into(now, &mut events);
        }
        for e in events.drain(..) {
            self.handle(e, ctx);
        }
        self.events = events;
        if now >= self.next_rtt {
            self.next_rtt = now + Duration::from_millis(500);
            let rtt = self.backend().and_then(|b| b.stats().ping_rtt);
            if let Some(v) = rtt
                && rtt != self.last_rtt
            {
                self.stats.rtt_ms.record(v.as_secs_f64() * 1e3);
            }
            self.last_rtt = rtt;
        }
    }

    fn handle(&mut self, e: Event, ctx: &mut Ctx<'_>) {
        let now = ctx.now;
        match e {
            Event::Connected { characters } => {
                self.stats.logins += 1;
                if let Some(t) = self.connect_started.take() {
                    self.stats
                        .login_ms
                        .record(now.duration_since(t).as_secs_f64() * 1e3);
                }
                if characters.iter().any(|c| c.name == self.spec.character) {
                    let character = self.spec.character.clone();
                    self.enter_started = Some(now);
                    self.phase = Phase::Entering { since: now };
                    self.send(now, Command::EnterWorld { character });
                } else {
                    self.fail(format!("character {} missing", self.spec.character));
                    self.disconnect(now, false);
                }
            }
            Event::WorldEntered { player, .. } => {
                self.stats.world_entries += 1;
                if let Some(t) = self.enter_started.take() {
                    self.stats
                        .enter_ms
                        .record(now.duration_since(t).as_secs_f64() * 1e3);
                }
                self.player = Some(player);
                self.phase = Phase::InWorld;
                // A new player entity starts Relaxed (`tessera-protocol.md` §7.3).
                self.stance = Stance::Relaxed;
                self.last_rtt = None;
                self.online.push_back((now, None));
                while self.online.len() > MAX_HISTORY {
                    self.online.pop_front();
                }
                self.plan = Plan::Pause {
                    until: now + Duration::from_millis(500),
                };
                // A new session holds no channel; the first join follows shortly.
                if self.channel != ChannelState::Out {
                    self.set_channel(now, ChannelState::Out);
                }
                self.next_channel = self
                    .channel_slot()
                    .map(|_| now + Duration::from_secs_f64(self.rng.range(1.0, 3.0)));
            }
            Event::Social(social) => self.channel_event(now, &social, ctx.ledger),
            Event::LocationReport {
                id,
                location,
                heading,
                cause,
                ..
            } if Some(id) == self.player => {
                let previous = self.located.then_some(self.pos);
                self.pos = location;
                self.heading = heading;
                self.vel = Vec3::ZERO;
                self.located = true;
                self.plan = Plan::Pause {
                    until: now + Duration::from_millis(300),
                };
                match cause {
                    LocationCause::Requested => return,
                    LocationCause::Entered => {
                        if !ctx.area.contains(&location) {
                            // Remote servers start bots at their default spawn: move into
                            // the area first.
                            let to = ctx.area.random_location(&mut self.rng);
                            self.record_truth(now, ctx.time, true);
                            self.teleport(now, to);
                            return;
                        }
                    }
                    LocationCause::Teleported => {
                        self.teleport_pending = None;
                        self.stats.teleports += 1;
                    }
                    LocationCause::Corrected => {
                        self.cheat_hold = None;
                        self.stats.corrections += 1;
                        let d = previous.map_or(0.0, |p| p.distance_xy(&location));
                        match self.open_cheat.as_mut() {
                            Some(c) if !c.answered => {
                                c.answered = true;
                                let back = c.from.distance_xy(&location);
                                if c.from.region == location.region && back <= CHEAT_POSE_TOLERANCE
                                {
                                    self.stats.cheats_corrected += 1;
                                } else {
                                    self.stats.cheats_bad_correction += 1;
                                    self.fail(format!(
                                        "cheat corrected to {back:.1} m from the place before it"
                                    ));
                                }
                            }
                            Some(c)
                                if c.from.region == location.region
                                    && c.from.distance_xy(&location) <= CHEAT_POSE_TOLERANCE =>
                            {
                                self.stats.cheat_extra_corrections += 1;
                            }
                            _ => {
                                self.corrections.push((now, d));
                                if self.corrections.len() > 4_096 {
                                    self.corrections.remove(0);
                                }
                            }
                        }
                    }
                    _ => {}
                }
                self.jumps.push_back(now);
                while self.jumps.len() > MAX_HISTORY {
                    self.jumps.pop_front();
                }
                self.record_truth(now, ctx.time, true);
            }
            Event::EntityCreated {
                id,
                kind,
                name,
                appearance,
                ..
            } => {
                if self.view.len() >= MAX_ENTITIES * 2 {
                    return;
                }
                let bot = (kind == EntityKind::Player)
                    .then(|| ctx.names.get(&name).copied())
                    .flatten();
                if let Some(b) = bot {
                    self.view_bot.insert(b, id);
                }
                self.view.insert(
                    id,
                    Seen {
                        name,
                        kind,
                        bot,
                        appearance: *appearance,
                    },
                );
            }
            Event::EntityDespawned { id } => {
                if let Some(s) = self.view.remove(&id)
                    && let Some(b) = s.bot
                    && self.view_bot.get(&b) == Some(&id)
                {
                    self.view_bot.remove(&b);
                }
            }
            Event::AppearanceChanged { id, appearance } => {
                if Some(id) == self.player {
                    self.appearance = Some(*appearance);
                    self.appearance_changed = Some(now);
                } else if let Some(s) = self.view.get_mut(&id) {
                    s.appearance = *appearance;
                }
            }
            Event::ChatMessage { channel, text, .. } => {
                if channel == ChatChannel::System {
                    self.stats.system_lines += 1;
                    match classify_system_line(&text) {
                        SystemLine::TeleportRefused => {
                            if self.teleport_pending.take().is_some() {
                                self.stats.teleports_refused += 1;
                                self.fail(text);
                            } else {
                                self.stats.refusals_unmatched += 1;
                                tracing::debug!(bot = self.spec.index, %text, "late refusal");
                            }
                        }
                        SystemLine::ChatRejected { benign } => {
                            self.stats.chat_rejected += 1;
                            ctx.ledger.rejected(self.spec.index);
                            if !benign {
                                self.fail(text);
                            }
                        }
                        SystemLine::ChannelRefused => {
                            // Membership is unchanged by a refusal (§6.4.6); a bot within
                            // the limits is never refused.
                            self.stats.channel_refusals += 1;
                            match self.channel {
                                ChannelState::Joining => self.set_channel(now, ChannelState::Out),
                                ChannelState::Leaving => {
                                    self.set_channel(now, ChannelState::Member);
                                }
                                _ => {}
                            }
                            self.next_channel = Some(now + Duration::from_secs(10));
                            self.fail(text);
                        }
                        SystemLine::Other => {}
                    }
                } else {
                    ctx.ledger.received(self.spec.index, &text, now);
                }
            }
            Event::Disconnected { reason } => {
                let failure = self.backend().and_then(|b| b.failure().cloned());
                self.end_session(now);
                match self.phase {
                    Phase::Closing {
                        reconnect: true, ..
                    } if !self.finishing => {
                        self.phase = Phase::Offline {
                            at: now + Duration::from_secs_f64(self.rng.range(1.0, 3.0)),
                        };
                    }
                    Phase::Closing { .. } => self.phase = Phase::Done,
                    _ if !self.finishing && admission_refusal(failure.as_ref()).is_some() => {
                        let wait = admission_refusal(failure.as_ref()).unwrap_or(1);
                        self.stats.admission_refusals += 1;
                        self.phase = Phase::Offline {
                            at: now + Duration::from_secs(u64::from(wait.clamp(1, 60))),
                        };
                    }
                    _ => {
                        self.stats.unexpected_disconnects += 1;
                        self.fail(format!("disconnected: {reason:?} ({failure:?})"));
                        self.phase = if self.finishing {
                            Phase::Done
                        } else {
                            Phase::Offline {
                                at: now + Duration::from_secs(5),
                            }
                        };
                    }
                }
            }
            _ => {}
        }
    }

    /// Advances the bot by `dt`: session timeouts, scheduled actions, movement, and the
    /// `Move` command with the new position.
    pub fn step(&mut self, now: Instant, dt: Duration, ctx: &mut Ctx<'_>) {
        match self.phase {
            Phase::Offline { at } => {
                if now >= at && !self.finishing {
                    self.connect(now);
                }
                return;
            }
            Phase::Connecting { since } if now.duration_since(since) > CONNECT_TIMEOUT => {
                self.fail("login timed out".into());
                self.end_session(now);
                self.phase = Phase::Offline {
                    at: now + Duration::from_secs(5),
                };
                return;
            }
            Phase::Entering { since } if now.duration_since(since) > ENTER_TIMEOUT => {
                self.fail("world entry timed out".into());
                self.end_session(now);
                self.phase = Phase::Offline {
                    at: now + Duration::from_secs(5),
                };
                return;
            }
            Phase::Closing { since, reconnect } if now.duration_since(since) > CLOSE_TIMEOUT => {
                self.stats.close_timeouts += 1;
                self.end_session(now);
                self.phase = if reconnect && !self.finishing {
                    Phase::Offline {
                        at: now + Duration::from_secs(1),
                    }
                } else {
                    Phase::Done
                };
                return;
            }
            Phase::InWorld => {}
            _ => return,
        }
        if !self.located {
            return;
        }
        self.adopt_server_speeds();
        self.channel_step(now, ctx.ledger);
        if let Some(c) = self.open_cheat
            && now.saturating_duration_since(c.at) > CHEAT_HOLD + CHEAT_ANSWER
        {
            self.open_cheat = None;
            if !c.answered {
                self.stats.cheats_unanswered += 1;
                tracing::info!(bot = self.spec.index, "cheat not corrected");
            }
        }
        if self.cheat_watch.is_some_and(|w| now > w.until) {
            self.cheat_watch = None;
        }
        if let Some(t) = self.teleport_pending {
            if now.duration_since(t) > TELEPORT_TIMEOUT {
                self.teleport_pending = None;
                self.stats.teleports_unanswered += 1;
                self.fail("teleport unanswered".into());
            }
            self.record_truth(now, ctx.time, false);
            return;
        }
        if let Some((until, to)) = self.cheat_hold {
            if now < until {
                let cmd = Command::Move {
                    location: to,
                    heading: self.heading,
                    velocity: Vec3::new(6.0, 0.0, 0.0),
                    on_ground: true,
                };
                self.send(now, cmd);
                self.record_truth(now, ctx.time, false);
                return;
            }
            self.cheat_hold = None;
        }
        if self.scheduled(now, ctx) {
            return;
        }
        self.advance(now, dt.min(Duration::from_millis(100)), ctx.area);
        let cmd = Command::Move {
            location: self.pos,
            heading: self.heading,
            velocity: self.vel,
            on_ground: true,
        };
        self.send(now, cmd);
        self.record_truth(now, ctx.time, false);
    }

    /// Takes the walk and run speeds the server sent for the player (`WALK_SPEED`,
    /// `RUN_SPEED`, `tessera-protocol.md` §7.3), so the bot stays within the server's limits.
    fn adopt_server_speeds(&mut self) {
        let Some(st) = self
            .player
            .and_then(|p| self.backend().and_then(|b| b.entity_state(p)))
        else {
            return;
        };
        let (walk, run) = (f64::from(st.walk_speed), f64::from(st.run_speed));
        if walk.is_finite() && run.is_finite() && walk > 0.0 && run >= walk && run <= 100.0 {
            self.behaviour.walk_speed = walk;
            self.behaviour.run_speed = run;
        }
    }

    /// Scheduled teleports, appearance changes and cheats. Returns true when the bot
    /// should not move this tick.
    fn scheduled(&mut self, now: Instant, ctx: &mut Ctx<'_>) -> bool {
        if let Some(iv) = self.behaviour.teleport_interval {
            let next = *self
                .next_teleport
                .get_or_insert_with(|| now + iv.mul_f64(self.rng.range(0.3, 1.0)));
            if now >= next {
                self.next_teleport = Some(now + jitter(&mut self.rng, iv));
                let (x, y) = if self.rng.chance(self.behaviour.far_teleport_fraction) {
                    // Far: more than R + 1 blocks from the old place, so every observer
                    // there must destroy the bot and it gets an InterestReset (§9.5).
                    let beyond = ctx.radius.saturating_add(2);
                    match ctx.area.random_point_beyond(
                        &mut self.rng,
                        self.pos.x,
                        self.pos.y,
                        beyond,
                    ) {
                        Some(p) => {
                            self.stats.far_teleports += 1;
                            p
                        }
                        None => {
                            self.stats.far_teleports_short += 1;
                            ctx.area.random_point(&mut self.rng)
                        }
                    }
                } else {
                    let (x, y) = ctx.area.random_point(&mut self.rng);
                    (
                        self.pos.x + (x - self.pos.x) * 0.1,
                        self.pos.y + (y - self.pos.y) * 0.1,
                    )
                };
                let to = Location::new(RegionId(ctx.area.region), x, y, ctx.area.z);
                self.teleport(now, to);
                return true;
            }
        }
        if let Some(iv) = self.behaviour.appearance_interval {
            let next = *self
                .next_appearance
                .get_or_insert_with(|| now + iv.mul_f64(self.rng.range(0.2, 1.0)));
            if now >= next {
                self.next_appearance = Some(now + jitter(&mut self.rng, iv));
                let can = self.backend().is_some_and(|b| {
                    b.server_info().is_some_and(|i| {
                        i.caps.0 & tessera_protocol::message::ServerCaps::APPEARANCE_CHANGE.0 != 0
                    })
                });
                if can {
                    let a = Appearance::new(Body::Character {
                        species: 1 + self.rng.below(8) as u32,
                        sex: 1 + self.rng.below(2) as u32,
                    });
                    if let Some(h) = self.backend().and_then(RemoteBackend::handle) {
                        if h.set_appearance(&a).is_ok() {
                            self.stats.appearance_changes += 1;
                        } else {
                            self.stats.send_errors += 1;
                        }
                    }
                }
                // A stance change too (a replicated property, §13.2 "State").
                let stance = if self.stance == Stance::Relaxed {
                    Stance::Ready
                } else {
                    Stance::Relaxed
                };
                if let Some(h) = self.backend().and_then(RemoteBackend::handle) {
                    if h.stance(stance).is_ok() {
                        self.stance = stance;
                        self.stats.stance_changes += 1;
                    } else {
                        self.stats.send_errors += 1;
                    }
                }
            }
        }
        if self.cheater
            && let Some(iv) = self.behaviour.cheat_interval
        {
            let next = *self
                .next_cheat
                .get_or_insert_with(|| now + iv.mul_f64(self.rng.range(0.3, 1.0)));
            if now >= next {
                self.next_cheat = Some(now + jitter(&mut self.rng, iv));
                // 40 m in one move: far above the 8.5 m allowance (§8.3), pointed into the
                // area so the clamp never shortens it much.
                let (cx, cy) = (
                    (ctx.area.min_x + ctx.area.max_x) / 2.0,
                    (ctx.area.min_y + ctx.area.max_y) / 2.0,
                );
                let toward = (cy - self.pos.y).atan2(cx - self.pos.x);
                let a = toward + self.rng.range(-1.0, 1.0);
                let mut to = self.pos;
                to.x = (to.x + CHEAT_DISTANCE * a.cos())
                    .clamp(ctx.area.min_x + 1.0, ctx.area.max_x - 1.0);
                to.y = (to.y + CHEAT_DISTANCE * a.sin())
                    .clamp(ctx.area.min_y + 1.0, ctx.area.max_y - 1.0);
                // Held so the backend's newest-move slot sends it; the truth stays where
                // the bot was, since the server must refuse the move.
                self.stats.cheats += 1;
                self.cheat_seq = self.cheat_seq.wrapping_add(1).max(1);
                self.vel = Vec3::ZERO;
                self.cheat_hold = Some((now + CHEAT_HOLD, to));
                if let Some(c) = self.open_cheat.take()
                    && !c.answered
                {
                    self.stats.cheats_unanswered += 1;
                }
                self.open_cheat = Some(OpenCheat {
                    at: now,
                    from: self.pos,
                    answered: false,
                });
                self.cheat_watch = Some(CheatWatch {
                    id: self.cheat_seq,
                    region: to.region.0,
                    x: to.x,
                    y: to.y,
                    until: now + CHEAT_WATCH,
                });
                return true;
            }
        }
        false
    }

    fn new_plan(&mut self, now: Instant, area: &Area) -> Plan {
        let b = &self.behaviour;
        if self.walker {
            if self.rng.chance(0.25) {
                return Plan::Pause {
                    until: now + Duration::from_secs_f64(self.rng.range(0.5, 3.0)),
                };
            }
            let speed = if self.rng.chance(b.run_fraction) {
                b.run_speed
            } else {
                b.walk_speed
            };
            // A leg of up to about 250 m, crossing block borders.
            let (tx, ty) = area.random_point(&mut self.rng);
            let f = (250.0
                / self
                    .pos
                    .distance_xy(&Location::new(self.pos.region, tx, ty, 0.0)))
            .min(1.0);
            Plan::Leg {
                x: self.pos.x + (tx - self.pos.x) * f,
                y: self.pos.y + (ty - self.pos.y) * f,
                speed,
            }
        } else if matches!(self.plan, Plan::Leg { .. }) || self.rng.chance(0.5) {
            Plan::Pause {
                until: now + Duration::from_secs_f64(self.rng.range(20.0, 60.0)),
            }
        } else {
            // A short walk now and then, so the server does not see an idle player.
            let a = self.rng.range(0.0, std::f64::consts::TAU);
            let x = (self.pos.x + 8.0 * a.cos()).clamp(area.min_x + 1.0, area.max_x - 1.0);
            let y = (self.pos.y + 8.0 * a.sin()).clamp(area.min_y + 1.0, area.max_y - 1.0);
            Plan::Leg {
                x,
                y,
                speed: b.walk_speed,
            }
        }
    }

    fn advance(&mut self, now: Instant, dt: Duration, area: &Area) {
        let dt = dt.as_secs_f64();
        match self.plan {
            Plan::Leg { x, y, speed } => {
                let (dx, dy) = (x - self.pos.x, y - self.pos.y);
                let dist = dx.hypot(dy);
                let step = speed * dt;
                if dist <= step || dist < 1e-6 {
                    self.pos.x = x;
                    self.pos.y = y;
                    self.vel = Vec3::ZERO;
                    self.plan = self.new_plan(now, area);
                } else {
                    let (ux, uy) = (dx / dist, dy / dist);
                    self.pos.x += ux * step;
                    self.pos.y += uy * step;
                    self.vel = Vec3::new((ux * speed) as f32, (uy * speed) as f32, 0.0);
                    self.heading = uy.atan2(ux) as f32;
                }
            }
            Plan::Pause { until } => {
                self.vel = Vec3::ZERO;
                if now >= until {
                    // Turn in place now and then before setting off.
                    if self.rng.chance(0.3) {
                        self.heading = self.rng.range(-3.1, 3.1) as f32;
                    }
                    self.plan = self.new_plan(now, area);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn area_geometry() {
        let a = Area::around(0, 127, 127, 4, 0.0);
        assert_eq!(a.min_x, 126.0 * 160.0);
        assert_eq!(a.max_x - a.min_x, 640.0);
        assert_eq!(a.blocks(), 4);
        let mut rng = Rng::new(3);
        for _ in 0..1000 {
            let l = a.random_location(&mut rng);
            assert!(a.contains(&l));
        }
        assert!(!a.contains(&Location::new(RegionId(1), 20_400.0, 20_400.0, 0.0)));
        // Clamped at the grid's edge.
        let e = Area::around(0, 0, 254, 5, 0.0);
        assert_eq!(e.min_x, 0.0);
        assert!(e.max_y <= 255.0 * 160.0);
        let one = Area::around(0, 10, 10, 0, 0.0);
        assert_eq!(one.max_x - one.min_x, 160.0);
    }

    /// Regression: with the old 4 × 4 default no point lay more than R + 1 = 3 blocks
    /// away, so far teleports never left interest range. The new default (8 = 2R + 4)
    /// allows it from every place.
    #[test]
    fn far_teleports_leave_interest_range() {
        let small = Area::around(0, 127, 127, 4, 0.0);
        assert!(!small.allows_leave(2));
        let mut rng = Rng::new(9);
        let (cx, cy) = (127.5 * 160.0, 127.5 * 160.0);
        assert_eq!(small.random_point_beyond(&mut rng, cx, cy, 4), None);
        let six = Area::around(0, 127, 127, 6, 0.0);
        assert!(six.allows_leave(2) && !six.allows_far_teleports(2));
        let a = Area::around(0, 127, 127, 8, 0.0);
        assert!(a.allows_leave(2) && a.allows_far_teleports(2));
        for _ in 0..2_000 {
            let (x, y) = a.random_point(&mut rng);
            let (tx, ty) = a
                .random_point_beyond(&mut rng, x, y, 4)
                .expect("an 8-block area always has a point 4 blocks away");
            let d = chebyshev(interest_block(x, y), interest_block(tx, ty));
            assert!(d >= 4, "{d}");
            assert!(a.contains(&Location::new(RegionId(0), tx, ty, 0.0)));
        }
    }

    #[test]
    fn system_lines_are_classified() {
        assert_eq!(
            classify_system_line("teleport refused: NOT_ALLOWED"),
            SystemLine::TeleportRefused
        );
        assert_eq!(
            classify_system_line("chat not sent: UNKNOWN_RECIPIENT: Botabc is not online"),
            SystemLine::ChatRejected { benign: true }
        );
        assert_eq!(
            classify_system_line("chat not sent: RATE_LIMITED"),
            SystemLine::ChatRejected { benign: false }
        );
        assert_eq!(
            classify_system_line("warning: server stopping soon"),
            SystemLine::Other
        );
        assert_eq!(
            classify_system_line("Region 0, block 127 127"),
            SystemLine::Other
        );
    }

    fn test_bot(t0: Instant) -> Bot {
        Bot::new(
            BotSpec {
                index: 0,
                account: "bot-0001".into(),
                password: Password::new("x"),
                character: "Botaaa".into(),
            },
            Endpoint {
                connect: "tessera://127.0.0.1:1".into(),
            },
            Behaviour::default(),
            1,
            t0,
        )
    }

    /// Regression: any system line while a teleport was pending used to count as a
    /// refusal (a bot failure that fails the run) and cleared the pending teleport.
    #[test]
    fn only_teleport_refusals_refuse_a_pending_teleport() {
        let t0 = Instant::now();
        let mut b = test_bot(t0);
        let names = HashMap::new();
        let mut ledger = ChatLedger::new(1, t0);
        let area = Area::around(0, 127, 127, 8, 0.0);
        let mut ctx = Ctx {
            now: t0,
            names: &names,
            ledger: &mut ledger,
            time: TimeBase::Estimated,
            area: &area,
            radius: 2,
        };
        let line = |text: &str| Event::ChatMessage {
            channel: ChatChannel::System,
            sender: None,
            text: text.into(),
        };
        b.teleport_pending = Some(t0);
        b.handle(line("chat not sent: UNKNOWN_RECIPIENT"), &mut ctx);
        b.handle(line("server stopping in 30 s: maintenance"), &mut ctx);
        b.handle(line("Tessera load test."), &mut ctx);
        assert!(b.teleport_pending.is_some());
        assert_eq!(b.stats.teleports_refused, 0);
        assert_eq!(b.stats.chat_rejected, 1);
        assert_eq!(b.stats.system_lines, 3);
        assert!(b.stats.failures.is_empty(), "{:?}", b.stats.failures);
        b.handle(line("teleport refused: NOT_ALLOWED"), &mut ctx);
        assert!(b.teleport_pending.is_none());
        assert_eq!(b.stats.teleports_refused, 1);
        assert_eq!(b.stats.failures.len(), 1);
        // A refusal with nothing pending is counted, not failed.
        b.handle(line("teleport refused: CANCELLED"), &mut ctx);
        assert_eq!(b.stats.refusals_unmatched, 1);
        assert_eq!(b.stats.failures.len(), 1);
        // A rejection of a bot within the limits is a failure.
        b.handle(line("chat not sent: RATE_LIMITED"), &mut ctx);
        assert_eq!(b.stats.failures.len(), 2);
    }

    #[test]
    fn password_is_redacted() {
        let b = test_bot(Instant::now());
        let text = format!("{:?}", b.spec);
        assert!(!text.contains("\"x\""), "{text}");
        assert!(text.contains("redacted"), "{text}");
    }

    #[test]
    fn time_base() {
        let base = Instant::now();
        let t = TimeBase::Shared {
            base,
            server_at_base: 1_000,
        };
        assert_eq!(
            t.server_us(base + Duration::from_millis(2), None),
            Some(3_000)
        );
        assert_eq!(TimeBase::Estimated.server_us(base, None), None);
    }

    #[test]
    fn intervals() {
        let t0 = Instant::now();
        let s = |ms| t0 + Duration::from_millis(ms);
        let mut b = test_bot(t0);
        b.online.push_back((s(0), Some(s(1_000))));
        b.online.push_back((s(2_000), None));
        assert!(b.online_during(s(100), s(900)));
        assert!(!b.online_during(s(900), s(2_100)));
        assert!(b.online_during(s(2_000), s(99_000)));
        b.jumps.push_back(s(500));
        assert!(b.jumped_during(s(400), s(600)));
        assert!(!b.jumped_during(s(600), s(700)));
        assert!(!b.in_world());
        assert!(!b.done());
        b.finish(t0);
        assert!(b.done());
    }
}
