// SPDX-License-Identifier: GPL-3.0-or-later
//! Desync detection (`test-server.md` §13.2).
//!
//! Ten times a second, for every observer bot *O* in the world and every other bot *E*, the
//! tracker compares what *O* shows with the truth:
//!
//! - **Position**: *O*'s interpolated position of *E* at *O*'s render time (estimated
//!   server time − *D*, from `tessera_net`'s own interpolation) against *E*'s true position
//!   at that server time; off by more than 1.0 m + 0.1 s × *E*'s true speed for 1 s.
//! - **Rest**: *E* at rest for 1 s and *O*'s position more than 0.05 m off for a further 1 s.
//! - **Missing** / **Ghost**: the interest-management rules of `tessera-protocol.md` §9.2
//!   applied to the true positions: *E* within *R* blocks for 2 s (plus the time the §9.4
//!   create budget needs to reach *E*, nearest first) but not created, while *O* is below its
//!   entity limit; or still created 2 s after it left *R* + 1 blocks, the region or the
//!   world.
//! - **State**: *O*'s appearance or stance of *E* differs from *E*'s for 2 s.
//! - **Motion** (`test-server.md` §15.2 implementer note): while *E* moves, *O*'s heading of
//!   *E* is more than 0.35 rad off, or *O*'s gait does not match *E*'s true speed, for 1 s.
//! - **Cheat**: while a cheating move of *E* is watched, *O* shows *E* within 5 m of the
//!   refused target (at once, no hold): the server relayed a move it must refuse.
//!
//! Corrections of bots that stayed within the limits and chat delivery problems are added
//! by the run. Each episode is counted once, when it reaches its duration.
//!
//! Pair state is kept only while a condition holds, so memory follows the number of
//! problems, not the square of the number of bots. The per-pair rules are the pure
//! function [`judge`], which the unit tests drive directly.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use tessera_net::Appearance;
use tessera_net::remote::{Gait, RemoteBackend};

use crate::bot::Bot;
use crate::metrics::Histogram;

/// The kinds of desync.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    /// Interpolated position too far from the truth.
    Position,
    /// Not settled on a resting entity.
    Rest,
    /// An entity in range not created.
    Missing,
    /// An entity out of range still created.
    Ghost,
    /// Appearance or stance differs.
    State,
    /// Heading or gait of a moving entity differs.
    Motion,
    /// An observer showed a cheating move the server must have refused or dropped.
    Cheat,
    /// A chat line late, lost, duplicated, out of order or misdelivered.
    Chat,
    /// A bot within the limits was corrected.
    Correction,
}

impl Kind {
    /// Every kind, in report order.
    pub const ALL: [Self; 9] = [
        Self::Position,
        Self::Rest,
        Self::Missing,
        Self::Ghost,
        Self::State,
        Self::Motion,
        Self::Cheat,
        Self::Chat,
        Self::Correction,
    ];

    /// Lower-case name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Position => "position",
            Self::Rest => "rest",
            Self::Missing => "missing",
            Self::Ghost => "ghost",
            Self::State => "state",
            Self::Motion => "motion",
            Self::Cheat => "cheat",
            Self::Chat => "chat",
            Self::Correction => "correction",
        }
    }
}

/// One desync episode.
#[derive(Debug, Clone, PartialEq)]
pub struct Episode {
    /// Kind.
    pub kind: Kind,
    /// The observer bot (for a correction or chat problem: the bot concerned).
    pub observer: usize,
    /// The entity's bot, if any.
    pub entity: Option<usize>,
    /// Seconds since the run started when the episode began.
    pub at_s: f64,
    /// Size: the largest error in metres (position, rest, cheat), radians or 0 (motion),
    /// or seconds (others).
    pub size: f64,
    /// The 1-minute load average when it was detected, if known.
    pub load: Option<f64>,
    /// Free text.
    pub note: String,
}

/// Parameters of the checks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DesyncConfig {
    /// Interest radius *R* in blocks.
    pub radius: u32,
    /// The observers' entity limit.
    pub max_entities: usize,
    /// New `EntityCreate`s per observer per server tick (`tessera-protocol.md` §9.4).
    pub creates_per_tick: u32,
    /// Server tick rate, Hz.
    pub tick_rate: u32,
}

impl DesyncConfig {
    /// The protocol's defaults: *R* = 2, 16 creates per tick at 30 Hz.
    #[must_use]
    pub fn new(radius: u32, max_entities: usize) -> Self {
        Self {
            radius,
            max_entities,
            creates_per_tick: 16,
            tick_rate: 30,
        }
    }

    /// The time the create budget needs to reach the `rank`-th nearest candidate
    /// (1-based): ⌈rank / creates_per_tick⌉ ticks (`test-server.md` §15.2 implementer note).
    #[must_use]
    pub fn create_allowance(&self, rank: usize) -> Duration {
        let per = usize::try_from(self.creates_per_tick.max(1)).unwrap_or(usize::MAX);
        let ticks = rank.div_ceil(per) as f64;
        Duration::from_secs_f64((ticks / f64::from(self.tick_rate.max(1))).min(3_600.0))
    }
}

/// Target sampling interval (10 Hz, §13.2).
pub const SAMPLE_INTERVAL: Duration = Duration::from_millis(100);
/// Position episodes: error above 1.0 m + 0.1 s × speed for this long.
pub const POSITION_HOLD: Duration = Duration::from_secs(1);
/// Rest: at rest this long, then off by more than [`REST_TOLERANCE`] for this long.
pub const REST_HOLD: Duration = Duration::from_secs(1);
/// Rest tolerance, metres.
pub const REST_TOLERANCE: f64 = 0.05;
/// Missing, ghost and state episodes: the condition holds this long.
pub const PRESENCE_HOLD: Duration = Duration::from_secs(2);
/// Motion episodes: heading or gait off for this long.
pub const MOTION_HOLD: Duration = Duration::from_secs(1);
/// Heading tolerance while moving, radians.
pub const HEADING_TOLERANCE: f64 = 0.35;
/// True speed from which heading and gait are judged, m/s.
pub const MOVING_SPEED: f64 = 0.5;
/// An observer's position of a cheater within this distance of the refused target shows
/// the cheat, metres (the target is 40 m away from the true position).
pub const CHEAT_SEEN_M: f64 = 5.0;
/// Most episodes listed individually (all are counted).
pub const MAX_EPISODES: usize = 2_000;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Timer {
    since: Option<Instant>,
    episode: Option<usize>,
}

impl Timer {
    /// Advances the timer: returns true when the condition just reached `hold`.
    fn update(&mut self, cond: bool, now: Instant, hold: Duration) -> bool {
        if !cond {
            self.since = None;
            self.episode = None;
            return false;
        }
        let since = *self.since.get_or_insert(now);
        self.episode.is_none() && now.saturating_duration_since(since) >= hold
    }

    fn reset(&mut self) {
        self.since = None;
        self.episode = None;
    }

    fn idle(&self) -> bool {
        self.since.is_none()
    }
}

/// The running checks of one observer–entity pair.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PairState {
    position: Timer,
    rest: Timer,
    missing: Timer,
    ghost: Timer,
    state: Timer,
    motion: Timer,
    /// The newest cheat (by the cheater's cheat number) already recorded for this pair.
    cheat_recorded: u32,
}

impl PairState {
    /// No check is running (the pair can be forgotten).
    #[must_use]
    pub fn idle(&self) -> bool {
        self.position.idle()
            && self.rest.idle()
            && self.missing.idle()
            && self.ghost.idle()
            && self.state.idle()
            && self.motion.idle()
            && self.cheat_recorded == 0
    }
}

/// A position comparison of one sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Compared {
    /// Horizontal error of the interpolated position, metres.
    pub error_m: f64,
    /// *E*'s true speed at the render time, m/s.
    pub true_speed: f64,
    /// *E* has been at rest for [`REST_HOLD`] at the render time.
    pub resting: bool,
    /// Heading error while *E* moves (at least [`MOVING_SPEED`]), radians.
    pub heading_error: Option<f64>,
    /// *O*'s gait of *E* differs from the one *E*'s true speed implies (while it moves).
    pub gait_differs: bool,
    /// While a cheat of *E* is watched: its number and the distance of *O*'s position of
    /// *E* from the refused target, metres.
    pub cheat: Option<(u32, f64)>,
}

/// What one sample shows of one observer–entity pair: the input of [`judge`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PairView {
    /// *E* is in the world with a known position.
    pub entity_in_world: bool,
    /// Block distance between the interest blocks of *O* and *E*; `None` when they are not
    /// in the same region or *E* is not in the world.
    pub distance: Option<u32>,
    /// *O* has *E* created.
    pub created: bool,
    /// *O* has *E* created more than once.
    pub duplicated: bool,
    /// *O* holds as many entities as its limit.
    pub observer_full: bool,
    /// Extra time before a missing *E* counts (the create budget, nearest first).
    pub missing_allowance: Duration,
    /// The server is behind its tick schedule: Missing is not judged.
    pub lagging: bool,
    /// What of *E*'s state *O* shows differently, if anything.
    pub state_differs: Option<&'static str>,
    /// The position comparison, when one was made.
    pub compared: Option<Compared>,
}

/// Where [`judge`] reports episodes.
pub trait Sink {
    /// A new episode; returns its index in the list, if listed.
    fn record(&mut self, kind: Kind, began: Instant, size: f64, note: String) -> Option<usize>;
    /// A running episode got larger.
    fn grow(&mut self, episode: Option<usize>, size: f64);
}

/// Applies the §13.2 rules to one sample of one pair (interest radius `r`).
pub fn judge(p: &mut PairState, v: &PairView, r: u32, now: Instant, sink: &mut dyn Sink) {
    // Missing.
    let in_range = v.distance.is_some_and(|d| d <= r);
    let missing = in_range && !v.created && !v.observer_full && !v.lagging;
    let since = p.missing.since;
    if p.missing
        .update(missing, now, PRESENCE_HOLD + v.missing_allowance)
    {
        let i = sink.record(
            Kind::Missing,
            since.unwrap_or(now),
            now.saturating_duration_since(since.unwrap_or(now))
                .as_secs_f64(),
            format!(
                "{} blocks away, not created",
                v.distance.unwrap_or(u32::MAX)
            ),
        );
        p.missing.episode = Some(i.unwrap_or(usize::MAX));
    }
    // Ghost: out of range, gone, or created twice.
    let gone = !v.entity_in_world || v.distance.is_none_or(|d| d > r.saturating_add(1));
    let ghost = v.duplicated || (v.created && gone);
    let since = p.ghost.since;
    if p.ghost.update(ghost, now, PRESENCE_HOLD) {
        let why = if v.duplicated {
            "created twice".to_owned()
        } else if !v.entity_in_world {
            "left the world".to_owned()
        } else {
            match v.distance {
                None => "other region".to_owned(),
                Some(d) => format!("{d} blocks away, still created"),
            }
        };
        let i = sink.record(
            Kind::Ghost,
            since.unwrap_or(now),
            PRESENCE_HOLD.as_secs_f64(),
            why,
        );
        p.ghost.episode = Some(i.unwrap_or(usize::MAX));
    }
    // State.
    let since = p.state.since;
    if p.state
        .update(v.state_differs.is_some(), now, PRESENCE_HOLD)
    {
        let i = sink.record(
            Kind::State,
            since.unwrap_or(now),
            PRESENCE_HOLD.as_secs_f64(),
            format!("{} differs", v.state_differs.unwrap_or("state")),
        );
        p.state.episode = Some(i.unwrap_or(usize::MAX));
    }
    // Position, rest, motion and cheat.
    let Some(c) = v.compared else {
        p.position.reset();
        p.rest.reset();
        p.motion.reset();
        return;
    };
    let limit = 1.0 + 0.1 * c.true_speed;
    let off = c.error_m > limit;
    let since = p.position.since;
    let ep = p.position.episode;
    if p.position.update(off, now, POSITION_HOLD) {
        let i = sink.record(
            Kind::Position,
            since.unwrap_or(now),
            c.error_m,
            format!("limit {limit:.2} m at {:.1} m/s", c.true_speed),
        );
        p.position.episode = Some(i.unwrap_or(usize::MAX));
    } else if off {
        sink.grow(ep, c.error_m);
    }
    let off = c.resting && c.error_m > REST_TOLERANCE;
    let since = p.rest.since;
    let ep = p.rest.episode;
    if p.rest.update(off, now, REST_HOLD) {
        let i = sink.record(
            Kind::Rest,
            since.unwrap_or(now),
            c.error_m,
            "resting entity not settled".into(),
        );
        p.rest.episode = Some(i.unwrap_or(usize::MAX));
    } else if off {
        sink.grow(ep, c.error_m);
    }
    let heading_off = c.heading_error.is_some_and(|h| h > HEADING_TOLERANCE);
    let since = p.motion.since;
    let ep = p.motion.episode;
    let size = c.heading_error.unwrap_or(0.0);
    if p.motion
        .update(heading_off || c.gait_differs, now, MOTION_HOLD)
    {
        let what = match (heading_off, c.gait_differs) {
            (true, true) => "heading and gait differ",
            (true, false) => "heading differs",
            _ => "gait differs",
        };
        let i = sink.record(
            Kind::Motion,
            since.unwrap_or(now),
            size,
            format!("{what} at {:.1} m/s", c.true_speed),
        );
        p.motion.episode = Some(i.unwrap_or(usize::MAX));
    } else if heading_off {
        sink.grow(ep, size);
    }
    if let Some((id, d)) = c.cheat
        && d <= CHEAT_SEEN_M
        && id != 0
        && p.cheat_recorded != id
    {
        p.cheat_recorded = id;
        sink.record(
            Kind::Cheat,
            now,
            d,
            format!("shown {d:.1} m from refused cheat target {id}"),
        );
    }
}

/// Smallest difference between two angles, radians, in [0, π].
#[must_use]
pub fn angle_diff(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(std::f64::consts::TAU);
    if d.is_finite() {
        d.min(std::f64::consts::TAU - d)
    } else {
        std::f64::consts::PI
    }
}

/// The gait the protocol derives from a horizontal speed (`tessera-protocol.md` §8.1):
/// Standing below 0.15 m/s, Running above the midpoint of the walk and run speeds.
#[must_use]
pub fn expected_gait(speed: f64, walk: f64, run: f64) -> Gait {
    if !speed.is_finite() || speed < 0.15 {
        Gait::Standing
    } else if speed > (walk + run) * 0.5 {
        Gait::Running
    } else {
        Gait::Walking
    }
}

/// The interest block of a position (`tessera-protocol.md` §9.1, no anchor): x and y
/// clamped into the grid, then the containing landblock.
#[must_use]
pub fn interest_block(x: f64, y: f64) -> (u8, u8) {
    let idx = |v: f64| -> u8 {
        let v = if v.is_finite() {
            v.clamp(0.0, 40_800.0)
        } else {
            0.0
        };
        (v / 160.0).floor().min(254.0) as u8
    };
    (idx(x), idx(y))
}

/// Chebyshev distance between two blocks.
#[must_use]
pub fn chebyshev(a: (u8, u8), b: (u8, u8)) -> u32 {
    u32::from(a.0.abs_diff(b.0)).max(u32::from(a.1.abs_diff(b.1)))
}

/// A fixed-size set of observer–entity pairs (one bit each).
#[derive(Debug, Clone)]
struct PairBits(Vec<u64>);

impl PairBits {
    fn new(pairs: usize) -> Self {
        Self(vec![0; pairs.div_ceil(64)])
    }

    fn set(&mut self, k: usize) {
        if let Some(w) = self.0.get_mut(k / 64) {
            *w |= 1 << (k % 64);
        }
    }

    fn get(&self, k: usize) -> bool {
        self.0.get(k / 64).is_some_and(|w| w & (1 << (k % 64)) != 0)
    }

    fn count(&self) -> usize {
        self.0.iter().map(|w| w.count_ones() as usize).sum()
    }
}

/// The episode list and counts.
#[derive(Debug)]
struct Log {
    start: Instant,
    counts: HashMap<Kind, u64>,
    episodes: Vec<Episode>,
    load: Option<f64>,
}

impl Log {
    fn record(
        &mut self,
        kind: Kind,
        observer: usize,
        entity: Option<usize>,
        began: Instant,
        size: f64,
        note: String,
    ) -> Option<usize> {
        *self.counts.entry(kind).or_insert(0) += 1;
        tracing::warn!(kind = kind.name(), observer, ?entity, size, %note, "desync");
        if self.episodes.len() >= MAX_EPISODES {
            return None;
        }
        self.episodes.push(Episode {
            kind,
            observer,
            entity,
            at_s: began.saturating_duration_since(self.start).as_secs_f64(),
            size,
            load: self.load,
            note,
        });
        Some(self.episodes.len() - 1)
    }

    fn grow(&mut self, idx: Option<usize>, size: f64) {
        if let Some(e) = idx.and_then(|i| self.episodes.get_mut(i))
            && size > e.size
        {
            e.size = size;
        }
    }
}

struct PairSink<'a> {
    log: &'a mut Log,
    observer: usize,
    entity: usize,
}

impl Sink for PairSink<'_> {
    fn record(&mut self, kind: Kind, began: Instant, size: f64, note: String) -> Option<usize> {
        self.log
            .record(kind, self.observer, Some(self.entity), began, size, note)
    }

    fn grow(&mut self, episode: Option<usize>, size: f64) {
        self.log.grow(episode, size);
    }
}

/// The tracker.
#[derive(Debug)]
pub struct DesyncTracker {
    cfg: DesyncConfig,
    n: usize,
    /// Running checks, by `observer * n + entity`; only pairs with a running check.
    pairs: HashMap<usize, PairState>,
    compared: PairBits,
    created_seen: PairBits,
    left: PairBits,
    log: Log,
    /// Position comparisons made.
    pub samples: u64,
    /// Horizontal position error of every comparison, metres.
    pub error_m: Histogram,
    /// Sampling rounds done.
    pub rounds: u64,
    /// Rounds in which Missing was not judged because the server was behind its schedule.
    pub lagging_rounds: u64,
    /// Time one sampling round took, ms.
    pub round_ms: Histogram,
    first_round: Option<Instant>,
    last_round: Option<Instant>,
    scratch_dups: Vec<u32>,
    scratch_rank: Vec<u32>,
}

impl DesyncTracker {
    /// A tracker for `n` bots.
    #[must_use]
    pub fn new(n: usize, cfg: DesyncConfig, start: Instant) -> Self {
        let pairs = n.saturating_mul(n);
        Self {
            cfg,
            n,
            pairs: HashMap::new(),
            compared: PairBits::new(pairs),
            created_seen: PairBits::new(pairs),
            left: PairBits::new(pairs),
            log: Log {
                start,
                counts: HashMap::new(),
                episodes: Vec::new(),
                load: None,
            },
            samples: 0,
            error_m: Histogram::new(),
            rounds: 0,
            lagging_rounds: 0,
            round_ms: Histogram::new(),
            first_round: None,
            last_round: None,
            scratch_dups: vec![0; n],
            scratch_rank: vec![0; n],
        }
    }

    /// Changes the interest radius (once a remote server's `HelloAck` is known).
    pub fn set_radius(&mut self, radius: u32) {
        self.cfg.radius = radius;
    }

    /// Changes the tick rate (once a remote server's `HelloAck` is known).
    pub fn set_tick_rate(&mut self, hz: u32) {
        self.cfg.tick_rate = hz.max(1);
    }

    /// The parameters in use.
    #[must_use]
    pub fn config(&self) -> DesyncConfig {
        self.cfg
    }

    /// The latest load average, noted on later episodes.
    pub fn set_load(&mut self, load: Option<f64>) {
        self.log.load = load;
    }

    /// The listed episodes (the first [`MAX_EPISODES`]).
    #[must_use]
    pub fn episodes(&self) -> &[Episode] {
        &self.log.episodes
    }

    /// Observer–entity pairs whose positions were compared at least once (the observer
    /// saw the entity move or stand).
    #[must_use]
    pub fn pairs_compared(&self) -> usize {
        self.compared.count()
    }

    /// Observer–entity pairs where the observer had the entity created and the entity
    /// later stood more than *R* + 1 blocks away: the leave side of interest management
    /// was exercised for them (and the Ghost check judged the destroy).
    #[must_use]
    pub fn pairs_left(&self) -> usize {
        self.left.count()
    }

    /// Pairs with a running check now.
    #[must_use]
    pub fn active_pairs(&self) -> usize {
        self.pairs.len()
    }

    /// Achieved sampling rate over the run, Hz (`None` before two rounds).
    #[must_use]
    pub fn sampling_rate(&self) -> Option<f64> {
        let (a, b) = (self.first_round?, self.last_round?);
        let span = b.saturating_duration_since(a).as_secs_f64();
        (self.rounds >= 2 && span > 0.0).then(|| (self.rounds - 1) as f64 / span)
    }

    /// Episodes of `kind`.
    #[must_use]
    pub fn count(&self, kind: Kind) -> u64 {
        self.log.counts.get(&kind).copied().unwrap_or(0)
    }

    /// All episodes.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.log.counts.values().sum()
    }

    /// Records an episode; returns its index in the list, if listed.
    pub fn record(
        &mut self,
        kind: Kind,
        observer: usize,
        entity: Option<usize>,
        began: Instant,
        size: f64,
        note: String,
    ) -> Option<usize> {
        self.log.record(kind, observer, entity, began, size, note)
    }

    /// Records an episode that began `at_s` seconds after the run started (chat problems,
    /// judged after the fact).
    pub fn record_at(
        &mut self,
        kind: Kind,
        observer: usize,
        entity: Option<usize>,
        at_s: f64,
        size: f64,
        note: String,
    ) {
        let at = if at_s.is_finite() {
            at_s.clamp(0.0, 1.0e9)
        } else {
            0.0
        };
        let began = self
            .log
            .start
            .checked_add(Duration::from_secs_f64(at))
            .unwrap_or(self.log.start);
        self.record(kind, observer, entity, began, size, note);
    }

    /// Counts `n` episodes of `kind` that are not listed (problems already summarised
    /// elsewhere), with one log line.
    pub fn add_unlisted(&mut self, kind: Kind, n: u64) {
        if n == 0 {
            return;
        }
        *self.log.counts.entry(kind).or_insert(0) += n;
        tracing::warn!(kind = kind.name(), n, "desync episodes not listed");
    }

    /// One sampling round over every observer and entity. `lagging`: the server is behind
    /// its tick schedule (Missing is not judged this round).
    pub fn sample(&mut self, now: Instant, bots: &[Bot], lagging: bool) {
        let began = Instant::now();
        self.rounds += 1;
        self.first_round.get_or_insert(now);
        self.last_round = Some(now);
        if lagging {
            self.lagging_rounds += 1;
        }
        let n = self.n.min(bots.len());
        let r = self.cfg.radius;
        let info: Vec<Option<(u16, (u8, u8))>> = bots
            .iter()
            .take(n)
            .map(|b| {
                b.in_world()
                    .then(|| (b.pos.region.0, interest_block(b.pos.x, b.pos.y)))
            })
            .collect();
        for (o, ob) in bots.iter().enumerate().take(n) {
            let Some((o_region, o_block)) = info[o] else {
                if !self.pairs.is_empty() {
                    for e in 0..n {
                        self.pairs.remove(&(o * self.n + e));
                    }
                }
                continue;
            };
            let backend = ob.backend();
            let render = backend.and_then(|b| b.render_time(now));
            for s in ob.view.values() {
                if let Some(b) = s.bot
                    && let Some(c) = self.scratch_dups.get_mut(b)
                {
                    *c += 1;
                }
            }
            let full = ob.view.len() >= self.cfg.max_entities;
            // Nearest-first ranks of the in-range candidates (the §9.4 create order).
            let mut cands: Vec<(f64, usize)> = Vec::new();
            for (e, i) in info.iter().enumerate() {
                if e != o
                    && let Some((reg, blk)) = i
                    && *reg == o_region
                    && chebyshev(o_block, *blk) <= r
                {
                    cands.push((bots[e].pos.distance_xy(&ob.pos), e));
                }
            }
            cands.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (rank, (_, e)) in cands.iter().enumerate() {
                self.scratch_rank[*e] = u32::try_from(rank + 1).unwrap_or(u32::MAX);
            }
            for (e, eb) in bots.iter().enumerate().take(n) {
                if e == o {
                    continue;
                }
                let k = o * self.n + e;
                let distance = info[e]
                    .filter(|(reg, _)| *reg == o_region)
                    .map(|(_, blk)| chebyshev(o_block, blk));
                let created = ob.view_bot.get(&e).copied();
                let duplicated = self.scratch_dups.get(e).is_some_and(|c| *c > 1);
                if created.is_some() {
                    self.created_seen.set(k);
                }
                if distance.is_some_and(|d| d > r.saturating_add(1)) && self.created_seen.get(k) {
                    self.left.set(k);
                }
                // Nothing to judge and nothing running: skip.
                if created.is_none()
                    && !duplicated
                    && distance.is_none_or(|d| d > r)
                    && !self.pairs.contains_key(&k)
                {
                    continue;
                }
                let e_in = info[e].is_some();
                let state_differs = match created {
                    Some(id) if e_in => {
                        // At minor 0 the server sends no slot and no look (protocol §3.1):
                        // when either side is there, only what minor 0 carries is compared.
                        let minor0 = [backend, eb.backend()]
                            .into_iter()
                            .any(|b| b.and_then(RemoteBackend::negotiated_minor) == Some(0));
                        let appearance = match (ob.view.get(&id), &eb.appearance) {
                            (Some(seen), Some(truth)) => {
                                !same_appearance(&seen.appearance, truth, minor0)
                            }
                            _ => false,
                        };
                        let stance = backend
                            .and_then(|b| b.entity_state(id))
                            .is_some_and(|s| s.stance != eb.stance);
                        if appearance {
                            Some("appearance")
                        } else if stance {
                            Some("stance")
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                let compared = match (created, render, backend) {
                    (Some(id), Some(rt), Some(b)) if distance.is_some() => b
                        .sample(id, now)
                        .zip(eb.trajectory.at(rt))
                        .filter(|(_, t)| t.region == o_region)
                        .map(|(s, t)| {
                            let (sx, sy) = (f64::from(s.position.x), f64::from(s.position.y));
                            let error_m = (sx - t.x).hypot(sy - t.y);
                            let resting = t.rest_since_us.is_some_and(|since| {
                                rt.saturating_sub(since) >= REST_HOLD.as_micros() as u64
                            });
                            let moving = t.speed >= MOVING_SPEED && !eb.cheating();
                            let heading_error = moving
                                .then(|| angle_diff(f64::from(s.heading), f64::from(t.heading)));
                            let (walk, run) = eb.speeds();
                            let gait_differs =
                                moving && s.motion.gait != expected_gait(t.speed, walk, run);
                            let cheat = eb
                                .cheat_watch
                                .filter(|w| now <= w.until && w.region == t.region)
                                .map(|w| (w.id, (sx - w.x).hypot(sy - w.y)));
                            Compared {
                                error_m,
                                true_speed: t.speed,
                                resting,
                                heading_error,
                                gait_differs,
                                cheat,
                            }
                        }),
                    _ => None,
                };
                if let Some(c) = &compared {
                    self.samples += 1;
                    self.compared.set(k);
                    self.error_m.record(c.error_m);
                }
                let rank = self.scratch_rank.get(e).copied().unwrap_or(0) as usize;
                let view = PairView {
                    entity_in_world: e_in,
                    distance,
                    created: created.is_some(),
                    duplicated,
                    observer_full: full,
                    missing_allowance: self.cfg.create_allowance(rank.max(1)),
                    lagging,
                    state_differs,
                    compared,
                };
                let mut p = self.pairs.remove(&k).unwrap_or_default();
                judge(
                    &mut p,
                    &view,
                    r,
                    now,
                    &mut PairSink {
                        log: &mut self.log,
                        observer: o,
                        entity: e,
                    },
                );
                if !p.idle() {
                    self.pairs.insert(k, p);
                }
            }
            for (_, e) in &cands {
                self.scratch_rank[*e] = 0;
            }
            for s in ob.view.values() {
                if let Some(b) = s.bot
                    && let Some(c) = self.scratch_dups.get_mut(b)
                {
                    *c = 0;
                }
            }
        }
        self.round_ms.record(began.elapsed().as_secs_f64() * 1e3);
    }
}

/// Whether two appearances agree; with `minor0`, only in what protocol minor 0 carries (no
/// item slot, no look; `tessera-protocol.md` §3.1), since an observer or a bot at minor 0
/// never learns the rest.
#[must_use]
pub fn same_appearance(a: &Appearance, b: &Appearance, minor0: bool) -> bool {
    if !minor0 {
        return a == b;
    }
    let strip = |x: &Appearance| {
        let mut x = x.clone();
        x.look = None;
        for e in &mut x.equipment {
            e.slot = None;
        }
        x
    };
    strip(a) == strip(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: a bot at minor 0 (every tenth) saw every other bot's appearance without
    /// item slots, and they saw its own with them, so each pair was a State desync.
    #[test]
    fn minor_zero_compares_what_minor_zero_carries() {
        let mut with = Appearance::new(tessera_net::Body::Character { species: 1, sex: 1 });
        let mut item = tessera_net::Equipped::new(tessera_net::DataId(0x7000_0001));
        let plain = {
            let mut a = with.clone();
            a.equipment.push(item);
            a
        };
        item.slot = Some(17);
        with.equipment.push(item);
        assert!(!same_appearance(&with, &plain, false));
        assert!(same_appearance(&with, &plain, true));
        let mut other = plain.clone();
        other.scale = 2.0;
        assert!(!same_appearance(&with, &other, true));
    }

    #[derive(Default)]
    struct Recorded(Vec<(Kind, f64, String)>);

    impl Sink for Recorded {
        fn record(
            &mut self,
            kind: Kind,
            _began: Instant,
            size: f64,
            note: String,
        ) -> Option<usize> {
            self.0.push((kind, size, note));
            Some(self.0.len() - 1)
        }

        fn grow(&mut self, episode: Option<usize>, size: f64) {
            if let Some(e) = episode.and_then(|i| self.0.get_mut(i))
                && size > e.1
            {
                e.1 = size;
            }
        }
    }

    impl Recorded {
        fn kinds(&self) -> Vec<Kind> {
            self.0.iter().map(|x| x.0).collect()
        }
    }

    const R: u32 = 2;

    /// A pair in range, created, compared with no error.
    fn healthy() -> PairView {
        PairView {
            entity_in_world: true,
            distance: Some(1),
            created: true,
            compared: Some(Compared {
                error_m: 0.1,
                true_speed: 6.0,
                resting: false,
                heading_error: Some(0.05),
                gait_differs: false,
                cheat: None,
            }),
            ..PairView::default()
        }
    }

    /// Feeds `v` every 100 ms from t0 for `ms` milliseconds; returns the recorded kinds.
    fn run(v: &PairView, ms: u64) -> (Recorded, PairState) {
        let t0 = Instant::now();
        let mut p = PairState::default();
        let mut sink = Recorded::default();
        let mut t = 0;
        while t <= ms {
            judge(&mut p, v, R, t0 + Duration::from_millis(t), &mut sink);
            t += 100;
        }
        (sink, p)
    }

    fn with_compared(f: impl FnOnce(&mut Compared)) -> PairView {
        let mut v = healthy();
        if let Some(c) = v.compared.as_mut() {
            f(c);
        }
        v
    }

    #[test]
    fn healthy_pair_records_nothing_and_is_idle() {
        let (s, p) = run(&healthy(), 10_000);
        assert!(s.0.is_empty(), "{:?}", s.kinds());
        assert!(p.idle());
    }

    #[test]
    fn position_fires_at_the_hold_not_before() {
        // 1.0 + 0.1 × 6 = 1.6 m limit.
        let v = with_compared(|c| c.error_m = 1.7);
        assert!(run(&v, 900).0.0.is_empty());
        let (s, _) = run(&v, 1_000);
        assert_eq!(s.kinds(), vec![Kind::Position]);
        // Counted once however long it lasts.
        assert_eq!(run(&v, 10_000).0.kinds(), vec![Kind::Position]);
        // Within the limit: nothing.
        assert!(
            run(&with_compared(|c| c.error_m = 1.55), 5_000)
                .0
                .0
                .is_empty()
        );
    }

    #[test]
    fn position_episode_grows() {
        let t0 = Instant::now();
        let mut p = PairState::default();
        let mut sink = Recorded::default();
        for (ms, err) in [
            (0, 2.0),
            (500, 2.0),
            (1_000, 2.0),
            (1_100, 9.0),
            (1_200, 3.0),
        ] {
            let v = with_compared(|c| c.error_m = err);
            judge(&mut p, &v, R, t0 + Duration::from_millis(ms), &mut sink);
        }
        assert_eq!(sink.0.len(), 1);
        assert_eq!(sink.0[0].1, 9.0);
    }

    #[test]
    fn rest_fires_only_for_a_resting_entity() {
        let v = with_compared(|c| {
            c.true_speed = 0.0;
            c.resting = true;
            c.error_m = 0.06;
            c.heading_error = None;
        });
        assert!(run(&v, 900).0.0.is_empty());
        assert_eq!(run(&v, 1_000).0.kinds(), vec![Kind::Rest]);
        let moving = with_compared(|c| c.error_m = 0.06);
        assert!(run(&moving, 5_000).0.0.is_empty());
        let settled = with_compared(|c| {
            c.true_speed = 0.0;
            c.resting = true;
            c.error_m = 0.04;
            c.heading_error = None;
        });
        assert!(run(&settled, 5_000).0.0.is_empty());
    }

    #[test]
    fn missing_waits_for_the_hold_and_the_create_budget() {
        let v = PairView {
            entity_in_world: true,
            distance: Some(2),
            ..PairView::default()
        };
        assert!(run(&v, 1_900).0.0.is_empty());
        assert_eq!(run(&v, 2_000).0.kinds(), vec![Kind::Missing]);
        // Out of range, a full observer, or a lagging server: not missing.
        for v in [
            PairView {
                distance: Some(3),
                ..v.clone()
            },
            PairView {
                observer_full: true,
                ..v.clone()
            },
            PairView {
                lagging: true,
                ..v.clone()
            },
            PairView {
                distance: None,
                ..v.clone()
            },
        ] {
            assert!(run(&v, 5_000).0.0.is_empty(), "{v:?}");
        }
        // The 40th nearest candidate at 16 creates per tick needs 3 ticks more.
        let cfg = DesyncConfig::new(2, 1024);
        let extra = cfg.create_allowance(40);
        assert_eq!(extra, Duration::from_secs_f64(0.1));
        assert_eq!(cfg.create_allowance(1023).as_millis(), 2_133);
        let late = PairView {
            missing_allowance: cfg.create_allowance(1023),
            ..v
        };
        assert!(run(&late, 4_100).0.0.is_empty());
        assert_eq!(run(&late, 4_200).0.kinds(), vec![Kind::Missing]);
    }

    #[test]
    fn ghost_fires_beyond_r_plus_one_gone_or_twice() {
        // R + 1 = 3 is still allowed (hysteresis).
        let at3 = PairView {
            distance: Some(3),
            compared: None,
            ..healthy()
        };
        assert!(run(&at3, 5_000).0.0.is_empty());
        for v in [
            PairView {
                distance: Some(4),
                compared: None,
                ..healthy()
            },
            PairView {
                distance: None,
                compared: None,
                ..healthy()
            },
            PairView {
                entity_in_world: false,
                distance: None,
                compared: None,
                ..healthy()
            },
            PairView {
                duplicated: true,
                ..healthy()
            },
        ] {
            assert!(run(&v, 1_900).0.0.is_empty(), "{v:?}");
            assert_eq!(run(&v, 2_000).0.kinds(), vec![Kind::Ghost], "{v:?}");
        }
    }

    #[test]
    fn state_fires_after_two_seconds() {
        let v = PairView {
            state_differs: Some("stance"),
            ..healthy()
        };
        assert!(run(&v, 1_900).0.0.is_empty());
        let (s, _) = run(&v, 2_000);
        assert_eq!(s.kinds(), vec![Kind::State]);
        assert_eq!(s.0[0].2, "stance differs");
    }

    #[test]
    fn motion_fires_on_heading_or_gait() {
        let heading = with_compared(|c| c.heading_error = Some(0.5));
        assert!(run(&heading, 900).0.0.is_empty());
        assert_eq!(run(&heading, 1_000).0.kinds(), vec![Kind::Motion]);
        let gait = with_compared(|c| c.gait_differs = true);
        assert_eq!(run(&gait, 1_000).0.kinds(), vec![Kind::Motion]);
        let within = with_compared(|c| c.heading_error = Some(0.3));
        assert!(run(&within, 5_000).0.0.is_empty());
        // At rest the heading is not judged.
        let resting = with_compared(|c| c.heading_error = None);
        assert!(run(&resting, 5_000).0.0.is_empty());
    }

    #[test]
    fn cheat_fires_at_once_and_once_per_cheat() {
        let seen = with_compared(|c| c.cheat = Some((3, 1.0)));
        let (s, p) = run(&seen, 0);
        assert_eq!(s.kinds(), vec![Kind::Cheat]);
        assert!(!p.idle());
        assert_eq!(run(&seen, 5_000).0.kinds(), vec![Kind::Cheat]);
        let far = with_compared(|c| c.cheat = Some((3, 30.0)));
        assert!(run(&far, 5_000).0.0.is_empty());
    }

    #[test]
    fn helpers() {
        assert_eq!(interest_block(-5.0, 50_000.0), (0, 254));
        assert_eq!(interest_block(20_400.0, 159.9), (127, 0));
        assert_eq!(interest_block(f64::NAN, 160.0), (0, 1));
        assert_eq!(chebyshev((1, 1), (4, 2)), 3);
        assert!((angle_diff(3.1, -3.1) - (std::f64::consts::TAU - 6.2)).abs() < 1e-9);
        assert_eq!(angle_diff(0.0, f64::NAN), std::f64::consts::PI);
        assert_eq!(expected_gait(0.1, 2.0, 6.0), Gait::Standing);
        assert_eq!(expected_gait(2.0, 2.0, 6.0), Gait::Walking);
        assert_eq!(expected_gait(6.0, 2.0, 6.0), Gait::Running);
        let mut b = PairBits::new(130);
        b.set(129);
        b.set(129);
        b.set(5_000);
        assert!(b.get(129) && !b.get(128));
        assert_eq!(b.count(), 1);
    }

    #[test]
    fn records_counts_and_unlisted() {
        let t0 = Instant::now();
        let mut d = DesyncTracker::new(2, DesyncConfig::new(2, 1024), t0);
        assert_eq!(d.total(), 0);
        let i = d.record(Kind::Ghost, 0, Some(1), t0, 2.0, "x".into());
        assert_eq!(i, Some(0));
        d.log.grow(i, 5.0);
        assert_eq!(d.episodes()[0].size, 5.0);
        d.add_unlisted(Kind::Chat, 4_000_000);
        assert_eq!(d.count(Kind::Chat), 4_000_000);
        assert_eq!(d.episodes().len(), 1);
        d.record_at(Kind::Chat, 0, None, f64::INFINITY, 0.0, "y".into());
        assert_eq!(d.total(), 4_000_002);
        d.sample(t0, &[], false);
        d.sample(t0 + Duration::from_millis(100), &[], true);
        assert_eq!(d.rounds, 2);
        assert_eq!(d.lagging_rounds, 1);
        assert!((d.sampling_rate().unwrap() - 10.0).abs() < 1e-6);
    }

    #[test]
    fn tracker_memory_is_sparse() {
        // 1,024 bots: three bit sets of 1 Mi pairs (128 KiB each), no per-pair timers.
        let d = DesyncTracker::new(1_024, DesyncConfig::new(2, 1024), Instant::now());
        assert_eq!(d.active_pairs(), 0);
        assert_eq!(d.compared.0.len(), 1_024 * 1_024 / 64);
    }
}
