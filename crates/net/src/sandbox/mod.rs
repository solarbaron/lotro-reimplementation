// SPDX-License-Identifier: GPL-3.0-or-later
//! An in-process fake world for offline development.
//!
//! [`SandboxBackend`] needs no network and no server. It:
//!
//! - answers `Connect` with a configurable character list,
//! - spawns the player at a configurable place on `EnterWorld`: a plain
//!   [`Transform`] in [`SandboxConfig::region`], or a [`SpawnPoint`]
//!   (region + landblock + local offset, an exact [`Location`], or a named
//!   place resolved by a [`WorldSource`]), optionally snapped to the ground;
//!   an interior position keeps its owning landblock ([`Anchor`]) and is never
//!   re-homed to the block its coordinates fall in,
//! - reports the player's region-qualified position
//!   ([`Event::LocationReport`]) on entry, after a teleport and on
//!   [`Command::QueryLocation`],
//! - teleports on [`Command::Teleport`] (ignoring moves the client sent before
//!   it saw the teleport's [`Event::LocationReport`]),
//! - with a [`WorldSource`], serves the static objects of the landblocks
//!   around the player ([`Event::StaticSpawned`]), see [`world`],
//! - gives the player an appearance ([`SandboxConfig::avatar`], an
//!   [`Event::AppearanceChanged`] right after [`Event::WorldEntered`]),
//! - spawns a few "wanderer" entities that walk in circles around the spawn
//!   point and reports their movement at a fixed tick rate; with
//!   [`SandboxConfig::wanderer_appearances`] they are animated characters
//!   ([`Event::EntityCreated`]),
//! - spawns a crowd of characters on a grid in front of the spawn
//!   ([`CrowdConfig`]): standing, walking or running in circles, with varied
//!   appearances and phases (a load test for the client's animation),
//! - echoes chat back (tells get a system notice instead),
//! - accepts player movement, correcting it back (an [`Event::EntityMoved`]
//!   and a precise [`Event::LocationReport`]) if it moves implausibly fast or
//!   into another region (a stand-in for server-side validation),
//! - optionally delays every event by a fixed latency,
//! - with a resolved scenario ([`SandboxConfig::scenario`], feature `scenario`), plays the
//!   scenario feed (`docs/specs/scenario-feed.md`): the scripted domain events of
//!   [`crate::domain`], timed steps, triggers on client commands and generic answers. The
//!   sandbox then spawns no wanderers or crowd, adds no latency and sends no welcome line;
//!   its character list comes from the scenario. [`SandboxBackend::scenario_status`] and
//!   [`SandboxBackend::scenario_counters`] report on the run.
//!
//! It is fully deterministic given the sequence of `now` values passed in
//! and the answers of the world source.
//!
//! ```
//! use std::time::Instant;
//! use tessera_net::sandbox::{SandboxBackend, SandboxConfig, SpawnPoint};
//! use tessera_net::{Command, Event, LocationCause, RegionId, Session};
//!
//! let config = SandboxConfig {
//!     spawn_point: Some(SpawnPoint::parse("1,126,140,80,20,400")?),
//!     ..SandboxConfig::default()
//! };
//! let mut session = Session::new(SandboxBackend::new(config)?);
//! let now = Instant::now();
//! session.send(now, Command::Connect)?;
//! let _ = session.poll(now);
//! session.send(now, Command::EnterWorld { character: "Wanderer".into() })?;
//! let events = session.poll(now);
//! assert!(events.iter().any(|e| matches!(
//!     e,
//!     Event::LocationReport { cause: LocationCause::Entered, location, .. }
//!         if location.region == RegionId(1) && location.x == 20_240.0
//! )));
//! let (here, _heading) = session.player_location().unwrap();
//! assert_eq!((here.y, here.z), (22_420.0, 400.0));
//! # Ok::<(), tessera_net::NetError>(())
//! ```

pub mod world;

use std::collections::VecDeque;
use std::f32::consts::TAU;
#[cfg(feature = "scenario")]
use std::sync::Arc;
use std::time::{Duration, Instant};

pub use world::{
    Landblock, Lookup, PlacedFigure, ResolvedPlace, SpawnPoint, StaticConfig, StaticEntity,
    StaticStats, WorldSource,
};

use crate::NetError;
use crate::domain::{CharacterCommand, CharacterEvent, Text};
use crate::model::{
    Anchor, Appearance, CharacterSummary, ChatChannel, Destination, DisconnectReason, EntityId,
    EntityKind, Location, LocationCause, RegionId, Transform, Vec3,
};
#[cfg(feature = "scenario")]
use crate::scenario::{ResolvedScenario, ScenarioCounters, ScenarioStatus, feed::Feed};
use crate::session::{Backend, Command, Event};
use world::{Statics, clamp_location, sanitize_text};

/// Upper bound on [`SandboxConfig::wanderers`], to keep the sandbox cheap.
pub const MAX_WANDERERS: usize = 1024;
/// Allowed range of [`SandboxConfig::wander_radius`], in metres.
pub const WANDER_RADIUS_RANGE: std::ops::RangeInclusive<f32> = 0.01..=1.0e5;
/// Largest [`SandboxConfig::wander_speed`] and
/// [`SandboxConfig::max_player_speed`], in metres per second.
pub const MAX_SPEED: f32 = 1.0e4;
/// Largest absolute value of each [`SandboxConfig::spawn`] coordinate, in
/// metres. With the other bounds this keeps every emitted transform finite.
pub const MAX_COORDINATE: f32 = 1.0e7;
/// Largest [`SandboxConfig::resolve_timeout`].
pub const MAX_RESOLVE_TIMEOUT: Duration = Duration::from_secs(600);
/// Largest [`SandboxConfig::latency`]. Every event is held for the latency,
/// so this also bounds how long the pending queue keeps growing.
pub const MAX_LATENCY: Duration = Duration::from_secs(10);
/// Upper bound on [`CrowdConfig::count`].
pub const MAX_CROWD: usize = 1024;
/// Most appearances in [`SandboxConfig::wanderer_appearances`] and
/// [`CrowdConfig::appearances`].
pub const MAX_APPEARANCES: usize = 256;
/// Allowed range of [`CrowdConfig::spacing`], in metres.
pub const CROWD_SPACING_RANGE: std::ops::RangeInclusive<f32> = 0.5..=100.0;
/// Largest [`CrowdConfig::ahead`], in metres.
pub const MAX_CROWD_AHEAD: f32 = 1000.0;

/// A crowd of characters placed by the sandbox in front of the spawn point
/// (Tessera's own load test for animated characters).
///
/// Members stand on a grid facing the spawn: `ceil(sqrt(2 · count))` columns
/// [`spacing`](Self::spacing) apart across the spawn heading, rows from
/// [`ahead`](Self::ahead) metres in front of the spawn outwards, so a camera
/// behind the player sees all of them. Member *i* stands still (*i* mod 3 =
/// 0), walks (1) or runs (2) in a circle around its grid point (radius 0.4 ×
/// or 0.8 × the spacing) at [`walk_speed`](Self::walk_speed) or
/// [`run_speed`](Self::run_speed), starting at its own phase, and has
/// appearance *i* mod `appearances.len()`. Heights are the spawn's; a client
/// with terrain puts them on the ground. Members follow the player on
/// teleports like the wanderers.
#[derive(Debug, Clone, PartialEq)]
pub struct CrowdConfig {
    /// Members, `0..=MAX_CROWD` (0: no crowd).
    pub count: usize,
    /// Appearances, cycled; not empty when `count > 0`, at most
    /// [`MAX_APPEARANCES`], each [valid](Appearance::is_valid).
    pub appearances: Vec<Appearance>,
    /// Grid spacing in metres, within [`CROWD_SPACING_RANGE`].
    pub spacing: f32,
    /// Distance of the first row in front of the spawn, `0..=MAX_CROWD_AHEAD`.
    pub ahead: f32,
    /// Walking speed in metres per second, in `(0, MAX_SPEED]`.
    pub walk_speed: f32,
    /// Running speed in metres per second, in `(0, MAX_SPEED]`.
    pub run_speed: f32,
}

impl Default for CrowdConfig {
    fn default() -> Self {
        Self {
            count: 0,
            appearances: Vec::new(),
            spacing: 3.0,
            ahead: 8.0,
            walk_speed: 2.0,
            run_speed: 6.0,
        }
    }
}

impl CrowdConfig {
    fn validate(&self) -> Result<(), NetError> {
        let speed_ok = |v: f32| v > 0.0 && v <= MAX_SPEED;
        if self.count > MAX_CROWD
            || (self.count > 0 && self.appearances.is_empty())
            || self.appearances.len() > MAX_APPEARANCES
            || !self.appearances.iter().all(Appearance::is_valid)
            || !CROWD_SPACING_RANGE.contains(&self.spacing)
            || !(0.0..=MAX_CROWD_AHEAD).contains(&self.ahead)
            || !speed_ok(self.walk_speed)
            || !speed_ok(self.run_speed)
        {
            return Err(NetError::InvalidConfig(
                "sandbox crowd settings out of range",
            ));
        }
        Ok(())
    }

    /// Columns of the grid.
    fn columns(&self) -> usize {
        // ceil(sqrt(2n)) without floating point surprises.
        let mut c = 1usize;
        while c * c < 2 * self.count {
            c += 1;
        }
        c
    }
}

/// Settings for [`SandboxBackend`].
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// Characters offered in [`Event::Connected`]. Must not be empty; each
    /// must pass [`is_valid_name`](crate::model::is_valid_name).
    pub characters: Vec<String>,
    /// Region of [`spawn`](Self::spawn) and of the fallback spawn when a
    /// [`spawn_point`](Self::spawn_point) cannot be resolved. `RegionId(0)`
    /// (the default) stands for "no world data".
    pub region: RegionId,
    /// Where the player appears without a [`spawn_point`](Self::spawn_point)
    /// (or when it cannot be resolved); coordinates within
    /// ±[`MAX_COORDINATE`]. Its heading is also the heading for named
    /// spawn points.
    pub spawn: Transform,
    /// Spawn here instead of at [`spawn`](Self::spawn). A
    /// [`Destination::Named`] point needs a [`WorldSource`]; without one, or
    /// when the source cannot resolve it within
    /// [`resolve_timeout`](Self::resolve_timeout), the player appears at
    /// [`spawn`](Self::spawn) and gets a system notice.
    pub spawn_point: Option<SpawnPoint>,
    /// How long to wait for a [`WorldSource`] that answers
    /// [`Lookup::Pending`] while resolving a spawn point or teleport (at
    /// most [`MAX_RESOLVE_TIMEOUT`]). Meanwhile the session stays in
    /// `EnteringWorld`, or in place for a teleport.
    pub resolve_timeout: Duration,
    /// Static entity streaming (used only with a [`WorldSource`]).
    pub statics: StaticConfig,
    /// Number of simulated moving entities (`0..=MAX_WANDERERS`).
    pub wanderers: usize,
    /// Radius in metres of each wanderer's circular path, within
    /// [`WANDER_RADIUS_RANGE`].
    pub wander_radius: f32,
    /// Wanderer walking speed in metres per second, in `(0, MAX_SPEED]`.
    pub wander_speed: f32,
    /// Interval between wanderer movement updates.
    pub tick_interval: Duration,
    /// Fixed delay applied to every event, at most [`MAX_LATENCY`].
    pub latency: Duration,
    /// Echo chat back to the player.
    pub echo_chat: bool,
    /// Player speeds above this (m/s) are treated as cheating/glitches and
    /// corrected with an [`Event::EntityMoved`] for the player. In
    /// `(0, MAX_SPEED]`. A free-flying world walker wants this high.
    pub max_player_speed: f32,
    /// The player's appearance, sent as [`Event::AppearanceChanged`] right
    /// after [`Event::WorldEntered`]. Must be [valid](Appearance::is_valid).
    pub avatar: Option<Appearance>,
    /// When not empty, wanderers are characters with these appearances
    /// (cycled) and spawn as [`Event::EntityCreated`] instead of
    /// [`Event::EntitySpawned`]. At most [`MAX_APPEARANCES`], each valid.
    pub wanderer_appearances: Vec<Appearance>,
    /// A crowd in front of the spawn (none by default).
    pub crowd: CrowdConfig,
    /// A resolved scenario to play (`docs/specs/scenario-feed.md`; none by default).
    /// With one, the sandbox uses no wanderers, crowd or latency, sends no welcome line,
    /// and offers the scenario's character list when it declares one.
    #[cfg(feature = "scenario")]
    pub scenario: Option<Arc<ResolvedScenario>>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            characters: vec!["Wanderer".to_owned()],
            region: RegionId(0),
            spawn: Transform::new(Vec3::ZERO, 0.0),
            spawn_point: None,
            resolve_timeout: Duration::from_secs(10),
            statics: StaticConfig::default(),
            wanderers: 2,
            wander_radius: 6.0,
            wander_speed: 1.5,
            tick_interval: Duration::from_millis(100),
            latency: Duration::ZERO,
            echo_chat: true,
            max_player_speed: 50.0,
            avatar: None,
            wanderer_appearances: Vec::new(),
            crowd: CrowdConfig::default(),
            #[cfg(feature = "scenario")]
            scenario: None,
        }
    }
}

impl SandboxConfig {
    fn validate(&self) -> Result<(), NetError> {
        if self.characters.is_empty() {
            return Err(NetError::InvalidConfig(
                "sandbox needs at least one character",
            ));
        }
        if !self
            .characters
            .iter()
            .all(|c| crate::model::is_valid_name(c))
        {
            return Err(NetError::InvalidConfig(
                "sandbox character name empty, too long or has control characters",
            ));
        }
        let p = self.spawn.position;
        if !self.spawn.is_finite() || [p.x, p.y, p.z].iter().any(|c| c.abs() > MAX_COORDINATE) {
            return Err(NetError::InvalidConfig(
                "sandbox spawn is not finite or too far out",
            ));
        }
        if let Some(point) = &self.spawn_point {
            point.validate()?;
        }
        if self.resolve_timeout > MAX_RESOLVE_TIMEOUT {
            return Err(NetError::InvalidConfig("sandbox resolve timeout too long"));
        }
        self.statics.validate()?;
        if self.wanderers > MAX_WANDERERS {
            return Err(NetError::InvalidConfig("too many sandbox wanderers"));
        }
        // NaN fails every comparison, so it is rejected here too.
        let speed_ok = |v: f32| v > 0.0 && v <= MAX_SPEED;
        if !WANDER_RADIUS_RANGE.contains(&self.wander_radius)
            || !speed_ok(self.wander_speed)
            || !speed_ok(self.max_player_speed)
        {
            return Err(NetError::InvalidConfig(
                "sandbox speeds or radius out of range",
            ));
        }
        if self.latency > MAX_LATENCY {
            return Err(NetError::InvalidConfig("sandbox latency too long"));
        }
        if self.tick_interval.is_zero() {
            return Err(NetError::InvalidConfig(
                "sandbox tick interval must be non-zero",
            ));
        }
        if self.avatar.as_ref().is_some_and(|a| !a.is_valid())
            || self.wanderer_appearances.len() > MAX_APPEARANCES
            || !self.wanderer_appearances.iter().all(Appearance::is_valid)
        {
            return Err(NetError::InvalidConfig("sandbox appearance out of range"));
        }
        self.crowd.validate()?;
        Ok(())
    }

    /// The fallback spawn location.
    fn default_location(&self) -> Location {
        Location::from_position(self.region, self.spawn.position)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Connected,
    /// `EnterWorld` accepted; waiting for the spawn point to resolve.
    Entering,
    InWorld,
    Closed,
}

/// A simulated entity: a wanderer or a crowd member.
#[derive(Debug)]
struct Actor {
    id: EntityId,
    /// Offset of the circle's centre (or of the standing spot) from the
    /// anchor (the spawn point, moved by teleports).
    offset: Vec3,
    /// Starting angle on the circle (radians).
    phase: f32,
    /// Circle radius in metres.
    radius: f32,
    /// Speed along the circle in metres per second; 0 stands still.
    speed: f32,
    /// Facing while standing still.
    heading: f32,
}

#[derive(Debug)]
struct Player {
    id: EntityId,
    name: String,
    transform: Transform,
    /// Precise position; `transform.position` is its `f32` narrowing.
    location: Location,
    /// Owner of an interior position (kept while it [owns](Anchor::owns)
    /// the player's location).
    anchor: Option<Anchor>,
    /// Teleports whose `Teleported` report has not been delivered yet; moves
    /// submitted meanwhile were sent from the old place and are ignored.
    unseen_teleports: u32,
    last_move: Instant,
}

/// A resolved place to put the player.
#[derive(Debug, Clone, Copy)]
struct Target {
    location: Location,
    heading: f32,
    anchor: Option<Anchor>,
    snap_to_ground: bool,
}

/// What a resolution is for.
#[derive(Debug)]
enum Purpose {
    Enter { character: String },
    Teleport,
}

#[derive(Debug)]
enum Stage {
    /// Resolving the destination's place name.
    Place,
    /// Waiting for the ground height under the target (if it snaps).
    Ground(Target),
}

/// A spawn point or teleport destination being resolved through the world
/// source.
#[derive(Debug)]
struct Resolution {
    purpose: Purpose,
    destination: Destination,
    /// Heading used when the destination has none (named places).
    default_heading: f32,
    started: Instant,
    stage: Stage,
}

/// The result of one resolution step.
enum Step {
    Wait,
    Done {
        target: Target,
        notice: Option<String>,
    },
    Failed(String),
}

/// The in-process world source; see the module documentation.
#[derive(Debug)]
pub struct SandboxBackend {
    config: SandboxConfig,
    phase: Phase,
    /// Events with the instant they become visible, in non-decreasing order.
    pending: VecDeque<(Instant, Event)>,
    /// Wanderers first, then crowd members.
    wanderers: Vec<Actor>,
    /// Where the wanderers' circles are anchored.
    wander_anchor: Vec3,
    /// The anchor moved (a teleport): standing actors are reported at the next tick.
    anchor_moved: bool,
    player: Option<Player>,
    world_start: Option<Instant>,
    last_tick: Option<Instant>,
    next_id: u64,
    source: Option<Box<dyn WorldSource>>,
    resolving: Option<Resolution>,
    statics: Statics,
    /// Reused buffer for events produced by the static streamer.
    scratch: Vec<Event>,
    /// The scenario feed, when the configuration has a scenario.
    #[cfg(feature = "scenario")]
    feed: Option<Feed>,
    /// Characters created in this sandbox without a scenario (`scenario-feed.md` §9.6),
    /// with the appearance of their `CreateCharacter`; in memory only.
    created: Vec<(String, Option<Appearance>)>,
}

/// Most names of the plain sandbox's character list (`scenario-feed.md` §9.6).
pub const MAX_SANDBOX_CHARACTERS: usize = 64;

impl SandboxBackend {
    /// Creates a sandbox without world data: no named places, no ground
    /// snapping, no static entities.
    ///
    /// # Errors
    ///
    /// [`NetError::InvalidConfig`] if `config` is out of range.
    pub fn new(config: SandboxConfig) -> Result<Self, NetError> {
        config.validate()?;
        #[cfg(feature = "scenario")]
        let mut config = config;
        #[cfg(feature = "scenario")]
        let feed = config.scenario.clone().map(|sc| {
            config.wanderers = 0;
            config.crowd = CrowdConfig::default();
            config.latency = Duration::ZERO;
            Feed::new(sc, &config.characters, config.echo_chat)
        });
        let statics = Statics::new(config.statics.clone());
        Ok(Self {
            config,
            phase: Phase::Idle,
            pending: VecDeque::new(),
            wanderers: Vec::new(),
            wander_anchor: Vec3::ZERO,
            anchor_moved: false,
            player: None,
            world_start: None,
            last_tick: None,
            next_id: 1,
            source: None,
            resolving: None,
            statics,
            scratch: Vec::new(),
            #[cfg(feature = "scenario")]
            feed,
            created: Vec::new(),
        })
    }

    /// The scenario's status (`docs/specs/scenario-feed.md` §3.4); `None` without a
    /// scenario. Through a [`Session`](crate::Session):
    /// `session.backend::<SandboxBackend>()`.
    #[cfg(feature = "scenario")]
    #[must_use]
    pub fn scenario_status(&self) -> Option<ScenarioStatus> {
        self.feed.as_ref().map(Feed::status)
    }

    /// The scenario's counters (skipped steps, dropped entries, spilled events); `None`
    /// without a scenario.
    #[cfg(feature = "scenario")]
    #[must_use]
    pub fn scenario_counters(&self) -> Option<ScenarioCounters> {
        self.feed.as_ref().map(Feed::counters)
    }

    /// The total weight of the events the scenario feed holds in its output queue
    /// (`docs/specs/scenario-feed.md` §7.4): never above 2,097,152 units. `None` without a
    /// scenario. A diagnostic for tests and fuzzing.
    #[cfg(feature = "scenario")]
    #[must_use]
    pub fn scenario_queued_weight(&self) -> Option<u64> {
        self.feed.as_ref().map(Feed::queued_weight)
    }

    /// The resolve timeout of the configuration in force
    /// ([`SandboxConfig::resolve_timeout`]).
    #[must_use]
    pub fn resolve_timeout(&self) -> Duration {
        self.config.resolve_timeout
    }

    /// The characters offered and accepted for `EnterWorld`.
    fn characters(&self) -> Vec<String> {
        #[cfg(feature = "scenario")]
        if let Some(f) = &self.feed {
            return f.characters().to_vec();
        }
        let mut names = self.config.characters.clone();
        names.extend(self.created.iter().map(|(n, _)| n.clone()));
        names
    }

    /// The plain sandbox's answer to `CreateCharacter` (`scenario-feed.md` §9.6): Tessera's
    /// own acknowledgement, no game rule.
    fn create_character(
        &mut self,
        now: Instant,
        name: String,
        appearance: Option<Box<Appearance>>,
        class: Option<u32>,
    ) {
        let names = self.characters();
        let refused = if names.contains(&name) {
            Some("A character with this name already exists.")
        } else if names.len() >= MAX_SANDBOX_CHARACTERS {
            Some("The character list is full.")
        } else {
            None
        };
        let event = match refused {
            Some(text) => CharacterEvent::CharacterRefused {
                name,
                reason: Text::Literal(text.to_owned()),
            },
            None => {
                let appearance = appearance.map(|a| *a);
                let event = CharacterEvent::CharacterInfo {
                    name: name.clone(),
                    body: appearance.as_ref().map(|a| a.body),
                    class,
                    level: None,
                    look: appearance.as_ref().and_then(|a| a.look.clone()),
                };
                self.created.push((name, appearance));
                event
            }
        };
        self.emit(now, Event::Character(event));
    }

    /// Whether the scenario feed takes in-world commands now.
    #[cfg(feature = "scenario")]
    fn feed_answers(&self) -> bool {
        self.feed.as_ref().is_some_and(Feed::answers_commands)
    }

    /// Runs the feed's due ticks and applies its player relocations.
    #[cfg(feature = "scenario")]
    fn poll_feed(&mut self, now: Instant) {
        let Some(feed) = self.feed.as_mut() else {
            return;
        };
        let mut out = std::mem::take(&mut self.scratch);
        out.clear();
        let failed = feed.poll(now, &mut out);
        let relocations = feed.take_relocations();
        for event in out.drain(..) {
            self.emit(now, event);
        }
        self.scratch = out;
        for (location, heading, anchor) in relocations {
            if let Some(player) = self.player.as_mut() {
                let transform = Transform::new(location.position(), heading);
                player.location = location;
                player.anchor = anchor;
                player.transform = transform;
                player.last_move = now;
                player.unseen_teleports = player.unseen_teleports.saturating_add(1);
                self.wander_anchor = transform.position;
                self.anchor_moved = true;
            }
        }
        if let Some(error) = failed {
            tracing::warn!(%error, "scenario feed stopped");
            self.phase = Phase::Closed;
            self.resolving = None;
        }
    }

    /// Creates a sandbox that uses `source` for named places, ground heights,
    /// static entities and location descriptions.
    ///
    /// # Errors
    ///
    /// [`NetError::InvalidConfig`] if `config` is out of range.
    pub fn with_world(
        config: SandboxConfig,
        source: impl WorldSource + 'static,
    ) -> Result<Self, NetError> {
        Self::with_boxed_world(config, Box::new(source))
    }

    /// Like [`with_world`](Self::with_world) with an already boxed source.
    ///
    /// # Errors
    ///
    /// [`NetError::InvalidConfig`] if `config` is out of range.
    pub fn with_boxed_world(
        config: SandboxConfig,
        source: Box<dyn WorldSource>,
    ) -> Result<Self, NetError> {
        let mut sandbox = Self::new(config)?;
        sandbox.source = Some(source);
        Ok(sandbox)
    }

    /// Counters of the static-entity streamer. Through a
    /// [`Session`](crate::Session): `session.backend::<SandboxBackend>()`.
    #[must_use]
    pub fn static_stats(&self) -> StaticStats {
        self.statics.stats()
    }

    /// The world source, if any (e.g. to inspect a test double).
    #[must_use]
    pub fn world_source(&self) -> Option<&dyn WorldSource> {
        self.source.as_deref()
    }

    /// Events not yet delivered (waiting for their latency to pass).
    #[must_use]
    pub fn queued_events(&self) -> usize {
        self.pending.len()
    }

    fn alloc_id(&mut self) -> EntityId {
        let id = EntityId(self.next_id);
        self.next_id += 1;
        id
    }

    fn emit(&mut self, now: Instant, event: Event) {
        let due = now.checked_add(self.config.latency).unwrap_or(now);
        // `now` is monotonic per the Backend contract, so due times are too.
        self.pending.push_back((due, event));
    }

    fn system(&mut self, now: Instant, text: impl Into<String>) {
        self.emit(
            now,
            Event::ChatMessage {
                channel: ChatChannel::System,
                sender: None,
                text: text.into(),
            },
        );
    }

    fn begin_enter(&mut self, now: Instant, character: String) {
        if !self.characters().contains(&character) {
            self.system(
                now,
                format!("No character named {character} on this account."),
            );
            self.emit(
                now,
                Event::Disconnected {
                    reason: DisconnectReason::ServerClosed("unknown character".to_owned()),
                },
            );
            self.phase = Phase::Closed;
            return;
        }
        self.phase = Phase::Entering;
        let point = self.config.spawn_point.clone().unwrap_or_else(|| {
            SpawnPoint::at(self.config.default_location(), self.config.spawn.heading)
        });
        self.resolving = Some(Resolution {
            purpose: Purpose::Enter { character },
            destination: point.destination,
            default_heading: self.config.spawn.heading,
            started: now,
            stage: Stage::Place,
        });
        self.advance_resolution(now);
    }

    fn begin_teleport(&mut self, now: Instant, destination: Destination) {
        let Some(player) = &self.player else {
            return;
        };
        let default_heading = player.transform.heading;
        if let Some(Resolution {
            purpose: Purpose::Teleport,
            ..
        }) = self.resolving.take()
        {
            // Every request gets exactly one answer.
            self.system(now, "Teleport cancelled: superseded by a newer request.");
        }
        self.resolving = Some(Resolution {
            purpose: Purpose::Teleport,
            default_heading,
            destination,
            started: now,
            stage: Stage::Place,
        });
        self.advance_resolution(now);
    }

    /// Runs the pending resolution as far as the world source allows.
    fn advance_resolution(&mut self, now: Instant) {
        let Some(mut res) = self.resolving.take() else {
            return;
        };
        let timed_out = now.saturating_duration_since(res.started) >= self.config.resolve_timeout;
        match self.resolve_step(&mut res, timed_out) {
            Step::Wait => self.resolving = Some(res),
            Step::Done { target, notice } => {
                match res.purpose {
                    Purpose::Enter { character } => self.finish_enter(now, character, target),
                    Purpose::Teleport => self.finish_teleport(now, target),
                }
                // Notices follow the world entry so they reach the world chat.
                if let Some(text) = notice {
                    self.system(now, text);
                }
            }
            Step::Failed(why) => match res.purpose {
                Purpose::Enter { character } => {
                    let target = Target {
                        location: self.config.default_location(),
                        heading: self.config.spawn.heading,
                        anchor: None,
                        snap_to_ground: false,
                    };
                    self.finish_enter(now, character, target);
                    self.system(
                        now,
                        format!("Cannot spawn there ({why}); using the default spawn point."),
                    );
                }
                Purpose::Teleport => self.system(now, format!("Teleport failed: {why}.")),
            },
        }
    }

    fn resolve_step(&mut self, res: &mut Resolution, timed_out: bool) -> Step {
        if let Stage::Place = res.stage {
            let exact = |location, heading, anchor| Target {
                location,
                heading,
                anchor,
                snap_to_ground: false,
            };
            let target = match &res.destination {
                Destination::Location { location, heading } => exact(*location, *heading, None),
                Destination::Ground { location, heading } => Target {
                    snap_to_ground: true,
                    ..exact(*location, *heading, None)
                },
                Destination::Anchored {
                    location,
                    heading,
                    anchor,
                } => exact(*location, *heading, Some(*anchor)),
                Destination::Named(name) => {
                    let Some(source) = self.source.as_mut() else {
                        return Step::Failed("no world data to look up place names".into());
                    };
                    match source.resolve_place(name) {
                        Lookup::Ready(place)
                            if place.location.is_valid()
                                && place.anchor.is_none_or(|a| a.owns(&place.location)) =>
                        {
                            let heading = place
                                .heading
                                .filter(|h| h.is_finite())
                                .unwrap_or(res.default_heading);
                            Target {
                                location: place.location,
                                heading,
                                anchor: place.anchor,
                                snap_to_ground: place.snap_to_ground && place.anchor.is_none(),
                            }
                        }
                        Lookup::Ready(_) => {
                            return Step::Failed(
                                "the world source gave an invalid location".into(),
                            );
                        }
                        Lookup::Pending if timed_out => {
                            return Step::Failed(format!(
                                "{} not resolved in time",
                                sanitize_text(name)
                            ));
                        }
                        Lookup::Pending => return Step::Wait,
                        Lookup::Unavailable(why) => return Step::Failed(sanitize_text(&why)),
                    }
                }
            };
            // Validated by `Destination::is_valid`, but stay defensive.
            if !target.location.is_valid() {
                return Step::Failed("invalid destination".into());
            }
            res.stage = Stage::Ground(target);
        }
        let Stage::Ground(mut target) = res.stage else {
            return Step::Wait;
        };
        if !target.heading.is_finite() {
            target.heading = 0.0;
        }
        let mut notice = None;
        if target.snap_to_ground
            && target.anchor.is_none()
            && let Some(source) = self.source.as_mut()
        {
            match source.ground_height(&target.location) {
                Lookup::Ready(z)
                    if z.is_finite() && z.abs() <= crate::model::MAX_LOCATION_COORDINATE =>
                {
                    target.location.z = z;
                }
                Lookup::Ready(_) => {
                    tracing::debug!("sandbox ignoring invalid ground height");
                }
                Lookup::Pending if timed_out => {
                    notice = Some("Ground height not known in time; height kept.".to_owned());
                }
                Lookup::Pending => return Step::Wait,
                Lookup::Unavailable(why) => {
                    tracing::debug!(why = %sanitize_text(&why), "sandbox: no ground height");
                }
            }
        }
        Step::Done { target, notice }
    }

    fn finish_enter(&mut self, now: Instant, character: String, target: Target) {
        let Target {
            location,
            heading,
            anchor,
            ..
        } = target;
        let id = self.alloc_id();
        let spawn = Transform::new(location.position(), heading);
        self.player = Some(Player {
            id,
            name: character.clone(),
            transform: spawn,
            location,
            anchor,
            unseen_teleports: 0,
            last_move: now,
        });
        self.emit(
            now,
            Event::WorldEntered {
                player: id,
                name: character.clone(),
                transform: spawn,
            },
        );
        self.emit(
            now,
            Event::LocationReport {
                id,
                location,
                heading,
                anchor,
                cause: LocationCause::Entered,
            },
        );
        #[cfg(feature = "scenario")]
        if self.feed.is_some() {
            let appearance = self
                .feed
                .as_ref()
                .and_then(|f| f.base_appearance_for(&character))
                .or_else(|| self.config.avatar.clone());
            if let Some(appearance) = appearance.clone() {
                self.emit(
                    now,
                    Event::AppearanceChanged {
                        id,
                        appearance: Box::new(appearance),
                    },
                );
            }
            if let Some(feed) = self.feed.as_mut() {
                feed.world_entered(now, id, &character, location, heading, appearance);
            }
            self.wander_anchor = spawn.position;
            self.world_start = Some(now);
            self.last_tick = Some(now);
            self.phase = Phase::InWorld;
            return;
        }
        // A character created in this sandbox enters with its creation appearance (§9.6).
        let created = self
            .created
            .iter()
            .find(|(n, _)| *n == character)
            .and_then(|(_, a)| a.clone());
        if let Some(appearance) = created.or_else(|| self.config.avatar.clone()) {
            self.emit(
                now,
                Event::AppearanceChanged {
                    id,
                    appearance: Box::new(appearance),
                },
            );
        }

        let n = self.config.wanderers;
        let ring = self.config.wander_radius * 2.0;
        self.wander_anchor = spawn.position;
        for i in 0..n {
            #[allow(clippy::cast_precision_loss)] // n <= MAX_WANDERERS
            let angle = TAU * i as f32 / n as f32;
            let offset = Vec3::new(ring * angle.cos(), ring * angle.sin(), 0.0);
            let id = self.alloc_id();
            self.wanderers.push(Actor {
                id,
                offset,
                phase: angle,
                radius: self.config.wander_radius,
                speed: self.config.wander_speed,
                heading: 0.0,
            });
        }
        self.add_crowd(heading);
        self.world_start = Some(now);
        self.last_tick = Some(now);
        for i in 0..self.wanderers.len() {
            let (transform, _) = self.wanderer_state(i, Duration::ZERO);
            let id = self.wanderers[i].id;
            let (name, appearance) = if i < n {
                let a = &self.config.wanderer_appearances;
                (
                    format!("Wanderer {}", i + 1),
                    (!a.is_empty()).then(|| a[i % a.len()].clone()),
                )
            } else {
                let c = &self.config.crowd;
                let k = i - n;
                (
                    format!("Crowd {}", k + 1),
                    Some(c.appearances[k % c.appearances.len()].clone()),
                )
            };
            let event = match appearance {
                Some(appearance) => Event::EntityCreated {
                    id,
                    kind: EntityKind::NonPlayer,
                    name,
                    class: None,
                    location: Location::from_position(location.region, transform.position),
                    heading: transform.heading,
                    appearance: Box::new(appearance),
                    tag: None,
                },
                None => Event::EntitySpawned {
                    id,
                    kind: EntityKind::NonPlayer,
                    name,
                    transform,
                },
            };
            self.emit(now, event);
        }
        self.system(now, "Welcome to the Tessera sandbox.");
        self.phase = Phase::InWorld;
        // Static entities follow from the next poll's streaming step.
    }

    fn finish_teleport(&mut self, now: Instant, target: Target) {
        let Target {
            location,
            heading,
            anchor,
            ..
        } = target;
        let Some(player) = self.player.as_mut() else {
            return;
        };
        let transform = Transform::new(location.position(), heading);
        player.location = location;
        player.anchor = anchor;
        player.transform = transform;
        player.last_move = now;
        player.unseen_teleports = player.unseen_teleports.saturating_add(1);
        let id = player.id;
        #[cfg(feature = "scenario")]
        if let Some(feed) = self.feed.as_mut() {
            feed.player_moved(location, heading);
        }
        // The wanderers follow the player.
        self.wander_anchor = transform.position;
        self.anchor_moved = true;
        self.emit(
            now,
            Event::LocationReport {
                id,
                location,
                heading,
                anchor,
                cause: LocationCause::Teleported,
            },
        );
        self.emit(
            now,
            Event::EntityMoved {
                id,
                transform,
                velocity: Vec3::ZERO,
            },
        );
    }

    fn report_location(&mut self, now: Instant) {
        let Some(player) = &self.player else {
            return;
        };
        let (id, location, heading, anchor) = (
            player.id,
            player.location,
            player.transform.heading,
            player.anchor,
        );
        self.emit(
            now,
            Event::LocationReport {
                id,
                location,
                heading,
                anchor,
                cause: LocationCause::Requested,
            },
        );
        let text = self
            .source
            .as_mut()
            .and_then(|s| s.describe(&location, heading, anchor))
            .map(|t| sanitize_text(&t))
            .filter(|t| !t.trim().is_empty());
        let text = text.unwrap_or_else(|| {
            format!(
                "Region {}, x {:.2}, y {:.2}, z {:.2}.",
                location.region.0, location.x, location.y, location.z
            )
        });
        self.system(now, text);
    }

    /// Streams static entities around the player (no-op without a source).
    fn update_statics(&mut self, now: Instant) {
        let (Some(source), Some(player)) = (self.source.as_mut(), self.player.as_ref()) else {
            return;
        };
        // An interior position streams around its owner, never around the
        // block its coordinates happen to fall in.
        let centre = player
            .anchor
            .map(|a| a.block)
            .or_else(|| Landblock::containing(&player.location));
        let mut events = std::mem::take(&mut self.scratch);
        let mut next_id = self.next_id;
        let mut alloc = || {
            let id = EntityId(next_id);
            next_id += 1;
            id
        };
        self.statics
            .update(centre, source.as_mut(), &mut alloc, &mut events);
        self.next_id = next_id;
        for event in events.drain(..) {
            self.emit(now, event);
        }
        self.scratch = events;
    }

    /// Adds the crowd's actors (after the wanderers), facing `heading`'s
    /// opposite (towards the spawn), on the grid of [`CrowdConfig`].
    fn add_crowd(&mut self, heading: f32) {
        let c = self.config.crowd.clone();
        if c.count == 0 {
            return;
        }
        let cols = c.columns();
        let (s, co) = heading.sin_cos();
        let forward = Vec3::new(co, s, 0.0);
        let right = Vec3::new(s, -co, 0.0);
        // The golden angle spreads the starting phases.
        const GOLDEN: f32 = 2.399_963;
        for i in 0..c.count {
            #[allow(clippy::cast_precision_loss)] // bounded by MAX_CROWD
            let (row, col, half) = (
                (i / cols) as f32,
                (i % cols) as f32,
                (cols - 1) as f32 / 2.0,
            );
            let offset = forward * (c.ahead + row * c.spacing) + right * ((col - half) * c.spacing);
            let (radius, speed) = match i % 3 {
                0 => (0.4 * c.spacing, 0.0),
                1 => (0.4 * c.spacing, c.walk_speed),
                _ => (0.8 * c.spacing, c.run_speed),
            };
            #[allow(clippy::cast_precision_loss)] // bounded by MAX_CROWD
            let phase = (GOLDEN * i as f32).rem_euclid(TAU);
            let id = self.alloc_id();
            self.wanderers.push(Actor {
                id,
                offset,
                phase,
                radius,
                speed,
                heading: heading + std::f32::consts::PI,
            });
        }
    }

    /// Transform and velocity of actor `i` at `elapsed` since world entry.
    fn wanderer_state(&self, i: usize, elapsed: Duration) -> (Transform, Vec3) {
        let w = &self.wanderers[i];
        if w.speed <= 0.0 {
            let h = w.heading.rem_euclid(TAU);
            return (Transform::new(self.wander_anchor + w.offset, h), Vec3::ZERO);
        }
        let r = w.radius;
        let omega = f64::from(w.speed) / f64::from(r);
        // Reduce the angle in f64 before narrowing: in f32, `elapsed` itself
        // loses sub-second precision after a few months of uptime.
        let theta =
            (f64::from(w.phase) + omega * elapsed.as_secs_f64()).rem_euclid(std::f64::consts::TAU);
        #[allow(clippy::cast_possible_truncation)] // in [0, TAU)
        let theta = theta as f32;
        let (s, c) = theta.sin_cos();
        let position = self.wander_anchor + w.offset + Vec3::new(r * c, r * s, 0.0);
        // Counter-clockwise motion: velocity is the tangent.
        let velocity = Vec3::new(-s, c, 0.0) * w.speed;
        let heading = theta + std::f32::consts::FRAC_PI_2;
        (Transform::new(position, heading % TAU), velocity)
    }

    fn player_move(&mut self, now: Instant, location: Location, heading: f32, velocity: Vec3) {
        let max_speed = f64::from(self.config.max_player_speed);
        let Some(player) = self.player.as_mut() else {
            return;
        };
        if player.unseen_teleports > 0 {
            // Sent from where the player was before the teleport.
            tracing::debug!("sandbox ignoring a move sent before the teleport was seen");
            return;
        }
        let dt = now
            .saturating_duration_since(player.last_move)
            .as_secs_f64();
        // Allow a little slack so a single late frame is not punished.
        let budget = max_speed * dt.max(0.1) * 1.5;
        let here = player.location;
        let moved = (location.x - here.x)
            .hypot(location.y - here.y)
            .hypot(location.z - here.z);
        let plausible = location.region == here.region && moved <= budget;
        if !plausible || !heading.is_finite() {
            tracing::debug!(moved, budget, "sandbox correcting implausible player move");
            let (id, current) = (player.id, player.transform);
            let (location, anchor) = (player.location, player.anchor);
            player.last_move = now;
            self.emit(
                now,
                Event::EntityMoved {
                    id,
                    transform: current,
                    velocity: Vec3::ZERO,
                },
            );
            self.emit(
                now,
                Event::LocationReport {
                    id,
                    location,
                    heading: current.heading,
                    anchor,
                    cause: LocationCause::Corrected,
                },
            );
        } else {
            player.location = clamp_location(location);
            player.transform = Transform::new(player.location.position(), heading);
            if player.anchor.is_some_and(|a| !a.owns(&player.location)) {
                // Moved out of the interior's reach.
                player.anchor = None;
            }
            player.last_move = now;
            let _ = velocity; // nobody else is watching in the sandbox
            #[cfg(feature = "scenario")]
            if let Some(feed) = self.feed.as_mut() {
                feed.player_moved(player.location, heading);
            }
        }
    }

    fn chat(&mut self, now: Instant, channel: ChatChannel, text: String) {
        if let ChatChannel::Tell { peer } = &channel {
            let notice = format!("{peer} is not in the sandbox.");
            self.system(now, notice);
            return;
        }
        if self.config.echo_chat {
            let sender = self.player.as_ref().map(|p| p.name.clone());
            self.emit(
                now,
                Event::ChatMessage {
                    channel,
                    sender,
                    text,
                },
            );
        }
    }

    fn tick(&mut self, now: Instant) {
        let (Some(start), Some(last)) = (self.world_start, self.last_tick) else {
            return;
        };
        if now.saturating_duration_since(last) < self.config.tick_interval {
            return;
        }
        // One update per entity per tick, positioned at `now`; after a long
        // stall we do not replay the missed ticks.
        self.last_tick = Some(now);
        let elapsed = now.saturating_duration_since(start);
        let all = std::mem::take(&mut self.anchor_moved);
        for i in 0..self.wanderers.len() {
            if !all && self.wanderers[i].speed <= 0.0 {
                continue; // standing still: nothing to report
            }
            let (transform, velocity) = self.wanderer_state(i, elapsed);
            let id = self.wanderers[i].id;
            self.emit(
                now,
                Event::EntityMoved {
                    id,
                    transform,
                    velocity,
                },
            );
        }
    }

    fn disconnect(&mut self, now: Instant) {
        if let Some(player) = self.player.take() {
            self.emit(now, Event::EntityDespawned { id: player.id });
        }
        self.emit(
            now,
            Event::Disconnected {
                reason: DisconnectReason::ClientRequested,
            },
        );
        self.phase = Phase::Closed;
        self.wanderers.clear();
        self.world_start = None;
        self.last_tick = None;
        self.resolving = None;
        if let Some(source) = self.source.as_mut() {
            self.statics.clear(source.as_mut());
        }
    }
}

impl Backend for SandboxBackend {
    fn name(&self) -> &str {
        "sandbox"
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn submit(&mut self, now: Instant, command: Command) -> Result<(), NetError> {
        match (self.phase, command) {
            (Phase::Idle, Command::Connect) => {
                let characters = self
                    .characters()
                    .into_iter()
                    .map(|name| CharacterSummary { name })
                    .collect();
                self.emit(now, Event::Connected { characters });
                self.phase = Phase::Connected;
                #[cfg(feature = "scenario")]
                if let Some(feed) = self.feed.as_mut() {
                    let mut out = Vec::new();
                    feed.on_connected(&mut out);
                    for event in out {
                        self.emit(now, event);
                    }
                }
            }
            #[cfg(feature = "scenario")]
            (Phase::Connected, Command::Character(c)) if self.feed.is_some() => {
                let mut out = Vec::new();
                if let Some(feed) = self.feed.as_mut() {
                    feed.pre_world_command(&c, &mut out);
                }
                for event in out {
                    self.emit(now, event);
                }
            }
            (
                Phase::Connected,
                Command::Character(CharacterCommand::CreateCharacter {
                    name,
                    appearance,
                    class,
                    ..
                }),
            ) => self.create_character(now, name, appearance, class),
            (Phase::Connected, Command::EnterWorld { character }) => {
                self.begin_enter(now, character);
            }
            (
                Phase::InWorld,
                Command::Move {
                    location,
                    heading,
                    velocity,
                },
            ) => self.player_move(now, location, heading, velocity),
            (Phase::InWorld, Command::Teleport { destination }) => {
                if destination.is_valid() {
                    self.begin_teleport(now, destination);
                } else {
                    self.system(now, "Teleport failed: invalid destination.");
                }
            }
            (Phase::InWorld, Command::QueryLocation) => self.report_location(now),
            #[cfg(feature = "scenario")]
            (
                Phase::InWorld,
                command @ (Command::Chat { .. }
                | Command::Inventory(_)
                | Command::Combat(_)
                | Command::Npc(_)
                | Command::Social(_)
                | Command::World(_)
                | Command::Progression(_)),
            ) if self.feed_answers() => {
                if let Some(feed) = self.feed.as_mut() {
                    feed.submit(now, command)?;
                }
            }
            (Phase::InWorld, Command::Chat { channel, text }) => self.chat(now, channel, text),
            (Phase::Closed, _) => return Err(NetError::Closed),
            (_, Command::Disconnect) => self.disconnect(now),
            (phase, command) => {
                tracing::debug!(?phase, command = command.name(), "sandbox ignoring command");
            }
        }
        Ok(())
    }

    fn poll(&mut self, now: Instant, out: &mut Vec<Event>) {
        if self.resolving.is_some() {
            self.advance_resolution(now);
        }
        if self.phase == Phase::InWorld {
            self.tick(now);
            self.update_statics(now);
            #[cfg(feature = "scenario")]
            self.poll_feed(now);
        }
        while let Some((due, _)) = self.pending.front() {
            if *due > now {
                break;
            }
            if let Some((_, event)) = self.pending.pop_front() {
                if let (
                    Event::LocationReport {
                        id,
                        cause: LocationCause::Teleported,
                        ..
                    },
                    Some(player),
                ) = (&event, self.player.as_mut())
                    && *id == player.id
                {
                    // The client sees the teleport now; its later moves
                    // start from the new place.
                    player.unseen_teleports = player.unseen_teleports.saturating_sub(1);
                }
                out.push(event);
            }
        }
    }
}
