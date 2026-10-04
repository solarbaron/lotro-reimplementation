// SPDX-License-Identifier: GPL-3.0-or-later
//! An in-process fake world for offline development.
//!
//! [`SandboxBackend`] needs no network and no server. It:
//!
//! - answers `Connect` with a configurable character list,
//! - spawns the player at a configurable [`Transform`] on `EnterWorld`,
//! - spawns a few "wanderer" entities that walk in circles around the spawn
//!   point and reports their movement at a fixed tick rate,
//! - echoes chat back (tells get a system notice instead),
//! - accepts player movement, correcting it back if it moves implausibly
//!   fast (a stand-in for server-side validation),
//! - optionally delays every event by a fixed latency.
//!
//! It is fully deterministic given the sequence of `now` values passed in.

use std::collections::VecDeque;
use std::f32::consts::TAU;
use std::time::{Duration, Instant};

use crate::NetError;
use crate::model::{
    CharacterSummary, ChatChannel, DisconnectReason, EntityId, EntityKind, Transform, Vec3,
};
use crate::session::{Backend, Command, Event};

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

/// Settings for [`SandboxBackend`].
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// Characters offered in [`Event::Connected`]. Must not be empty; each
    /// must pass [`is_valid_name`](crate::model::is_valid_name).
    pub characters: Vec<String>,
    /// Where the player appears; coordinates within ±[`MAX_COORDINATE`].
    pub spawn: Transform,
    /// Number of simulated moving entities (`0..=MAX_WANDERERS`).
    pub wanderers: usize,
    /// Radius in metres of each wanderer's circular path, within
    /// [`WANDER_RADIUS_RANGE`].
    pub wander_radius: f32,
    /// Wanderer walking speed in metres per second, in `(0, MAX_SPEED]`.
    pub wander_speed: f32,
    /// Interval between wanderer movement updates.
    pub tick_interval: Duration,
    /// Fixed delay applied to every event.
    pub latency: Duration,
    /// Echo chat back to the player.
    pub echo_chat: bool,
    /// Player speeds above this (m/s) are treated as cheating/glitches and
    /// corrected with an [`Event::EntityMoved`] for the player. In
    /// `(0, MAX_SPEED]`.
    pub max_player_speed: f32,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            characters: vec!["Wanderer".to_owned()],
            spawn: Transform::new(Vec3::ZERO, 0.0),
            wanderers: 2,
            wander_radius: 6.0,
            wander_speed: 1.5,
            tick_interval: Duration::from_millis(100),
            latency: Duration::ZERO,
            echo_chat: true,
            max_player_speed: 50.0,
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
        if self.tick_interval.is_zero() {
            return Err(NetError::InvalidConfig(
                "sandbox tick interval must be non-zero",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Connected,
    InWorld,
    Closed,
}

#[derive(Debug)]
struct Wanderer {
    id: EntityId,
    center: Vec3,
    phase: f32,
}

#[derive(Debug)]
struct Player {
    id: EntityId,
    name: String,
    transform: Transform,
    last_move: Instant,
}

/// The in-process world source; see the module documentation.
#[derive(Debug)]
pub struct SandboxBackend {
    config: SandboxConfig,
    phase: Phase,
    /// Events with the instant they become visible, in non-decreasing order.
    pending: VecDeque<(Instant, Event)>,
    wanderers: Vec<Wanderer>,
    player: Option<Player>,
    world_start: Option<Instant>,
    last_tick: Option<Instant>,
    next_id: u64,
}

impl SandboxBackend {
    /// Creates a sandbox.
    ///
    /// # Errors
    ///
    /// [`NetError::InvalidConfig`] if `config` is out of range.
    pub fn new(config: SandboxConfig) -> Result<Self, NetError> {
        config.validate()?;
        Ok(Self {
            config,
            phase: Phase::Idle,
            pending: VecDeque::new(),
            wanderers: Vec::new(),
            player: None,
            world_start: None,
            last_tick: None,
            next_id: 1,
        })
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

    fn enter_world(&mut self, now: Instant, character: String) {
        if !self.config.characters.contains(&character) {
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
        let id = self.alloc_id();
        let spawn = self.config.spawn;
        self.player = Some(Player {
            id,
            name: character.clone(),
            transform: spawn,
            last_move: now,
        });
        self.emit(
            now,
            Event::WorldEntered {
                player: id,
                name: character,
                transform: spawn,
            },
        );

        let n = self.config.wanderers;
        let ring = self.config.wander_radius * 2.0;
        for i in 0..n {
            #[allow(clippy::cast_precision_loss)] // n <= MAX_WANDERERS
            let angle = TAU * i as f32 / n as f32;
            let center = spawn.position + Vec3::new(ring * angle.cos(), ring * angle.sin(), 0.0);
            let id = self.alloc_id();
            self.wanderers.push(Wanderer {
                id,
                center,
                phase: angle,
            });
        }
        self.world_start = Some(now);
        self.last_tick = Some(now);
        for i in 0..self.wanderers.len() {
            let (transform, _) = self.wanderer_state(i, Duration::ZERO);
            let id = self.wanderers[i].id;
            self.emit(
                now,
                Event::EntitySpawned {
                    id,
                    kind: EntityKind::NonPlayer,
                    name: format!("Wanderer {}", i + 1),
                    transform,
                },
            );
        }
        self.system(now, "Welcome to the Tessera sandbox.");
        self.phase = Phase::InWorld;
    }

    /// Transform and velocity of wanderer `i` at `elapsed` since world entry.
    fn wanderer_state(&self, i: usize, elapsed: Duration) -> (Transform, Vec3) {
        let w = &self.wanderers[i];
        let r = self.config.wander_radius;
        let omega = f64::from(self.config.wander_speed) / f64::from(r);
        // Reduce the angle in f64 before narrowing: in f32, `elapsed` itself
        // loses sub-second precision after a few months of uptime.
        let theta =
            (f64::from(w.phase) + omega * elapsed.as_secs_f64()).rem_euclid(std::f64::consts::TAU);
        #[allow(clippy::cast_possible_truncation)] // in [0, TAU)
        let theta = theta as f32;
        let (s, c) = theta.sin_cos();
        let position = w.center + Vec3::new(r * c, r * s, 0.0);
        // Counter-clockwise motion: velocity is the tangent.
        let velocity = Vec3::new(-s, c, 0.0) * self.config.wander_speed;
        let heading = theta + std::f32::consts::FRAC_PI_2;
        (Transform::new(position, heading % TAU), velocity)
    }

    fn player_move(&mut self, now: Instant, transform: Transform, velocity: Vec3) {
        let max_speed = self.config.max_player_speed;
        let Some(player) = self.player.as_mut() else {
            return;
        };
        let dt = now
            .saturating_duration_since(player.last_move)
            .as_secs_f32();
        // Allow a little slack so a single late frame is not punished.
        let budget = max_speed * dt.max(0.1) * 1.5;
        let moved = transform.position.distance(player.transform.position);
        if moved > budget {
            tracing::debug!(moved, budget, "sandbox correcting implausible player move");
            let (id, current) = (player.id, player.transform);
            player.last_move = now;
            self.emit(
                now,
                Event::EntityMoved {
                    id,
                    transform: current,
                    velocity: Vec3::ZERO,
                },
            );
        } else {
            player.transform = transform;
            player.last_move = now;
            let _ = velocity; // nobody else is watching in the sandbox
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
        for i in 0..self.wanderers.len() {
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
}

impl Backend for SandboxBackend {
    fn name(&self) -> &str {
        "sandbox"
    }

    fn submit(&mut self, now: Instant, command: Command) -> Result<(), NetError> {
        match (self.phase, command) {
            (Phase::Idle, Command::Connect) => {
                let characters = self
                    .config
                    .characters
                    .iter()
                    .map(|name| CharacterSummary { name: name.clone() })
                    .collect();
                self.emit(now, Event::Connected { characters });
                self.phase = Phase::Connected;
            }
            (Phase::Connected, Command::EnterWorld { character }) => {
                self.enter_world(now, character);
            }
            (
                Phase::InWorld,
                Command::Move {
                    transform,
                    velocity,
                },
            ) => self.player_move(now, transform, velocity),
            (Phase::InWorld, Command::Chat { channel, text }) => self.chat(now, channel, text),
            (Phase::Closed, _) => return Err(NetError::Closed),
            (_, Command::Disconnect) => {
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
            }
            (phase, command) => {
                tracing::debug!(?phase, command = command.name(), "sandbox ignoring command");
            }
        }
        Ok(())
    }

    fn poll(&mut self, now: Instant, out: &mut Vec<Event>) {
        if self.phase == Phase::InWorld {
            self.tick(now);
        }
        while let Some((due, _)) = self.pending.front() {
            if *due > now {
                break;
            }
            if let Some((_, event)) = self.pending.pop_front() {
                out.push(event);
            }
        }
    }
}
