// SPDX-License-Identifier: GPL-3.0-or-later
//! Movement and interest helpers (`tessera-protocol.md` §8, §9): serial-number comparison,
//! the client's send schedule, the server's movement validator, and interest blocks.

use std::time::Duration;

use crate::limits::{
    ALLOWANCE_WINDOW, DEFAULT_RUN_SPEED, DEFAULT_TICK_RATE, EXTENT_MAX, EXTENT_MIN, GRID_EXTENT,
    HORIZONTAL_TOLERANCE_FACTOR, LANDBLOCK_SIZE, MAX_CORRECTIONS_PER_SECOND, MAX_DOWN_SPEED,
    MAX_LANDBLOCK, MAX_UP_SPEED, REST_REDUNDANCY,
};
use crate::message::{LocationCause, LocationReport, MoveInput};
use crate::rate::SlidingWindow;
use crate::types::{Anchor, Location, Motion, Pose, Vec3};

/// RFC 1982 serial comparison for u32: whether `a` is newer than `b`.
#[must_use]
pub fn serial_newer(a: u32, b: u32) -> bool {
    a != b && (a.wrapping_sub(b) as i32) > 0
}

/// The client's `MoveInput` schedule (§8.2), driven once per 30 Hz simulation tick.
///
/// While moving or turning it sends every second tick (15 Hz); it sends at once when the gait
/// changes, on starting or stopping, and when a teleport or correction is adopted; after
/// stopping it repeats the rest state in the next 3 ticks; at rest it sends a 1 Hz heartbeat.
/// It never sends more than once per tick.
#[derive(Debug, Clone, Default)]
pub struct MoveScheduler {
    last_send: Option<u64>,
    was_moving: bool,
    last_motion: Option<Motion>,
    redundancy: u32,
}

impl MoveScheduler {
    /// A scheduler that sends on its first tick.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether to send a `MoveInput` on simulation tick `tick`. `moving` is true while the
    /// avatar moves or turns; `forced` is true on the tick a teleport or correction was
    /// adopted.
    pub fn on_tick(&mut self, tick: u64, moving: bool, motion: Motion, forced: bool) -> bool {
        let gait_changed = self.last_motion.is_some_and(|m| m != motion);
        let started = moving && !self.was_moving;
        let stopped = !moving && self.was_moving;
        self.was_moving = moving;
        self.last_motion = Some(motion);
        if stopped {
            self.redundancy = REST_REDUNDANCY;
        }
        let since = self.last_send.map(|t| tick.saturating_sub(t));
        if since == Some(0) {
            return false;
        }
        let send = if forced || gait_changed || started || stopped || since.is_none() {
            true
        } else if moving {
            since.is_some_and(|s| s >= 2)
        } else if self.redundancy > 0 {
            self.redundancy -= 1;
            true
        } else {
            since.is_some_and(|s| s >= u64::from(DEFAULT_TICK_RATE))
        };
        if send {
            self.last_send = Some(tick);
        }
        send
    }
}

/// Tolerances of the server's movement check (§8.3 defaults; `test-server.md` may change
/// them).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MovementLimits {
    /// The player's `RUN_SPEED`, m/s; *V*ₕ = 1.25 × this.
    pub run_speed: f32,
    /// *V*ᵤ, m/s.
    pub up_speed: f64,
    /// *V*_d, m/s.
    pub down_speed: f64,
    /// *W*, the burst window.
    pub window: Duration,
    /// Lowest x or y accepted, metres.
    pub extent_min: f64,
    /// Highest x or y accepted, metres.
    pub extent_max: f64,
}

impl Default for MovementLimits {
    fn default() -> Self {
        Self {
            run_speed: DEFAULT_RUN_SPEED,
            up_speed: MAX_UP_SPEED,
            down_speed: MAX_DOWN_SPEED,
            window: ALLOWANCE_WINDOW,
            extent_min: EXTENT_MIN,
            extent_max: EXTENT_MAX,
        }
    }
}

impl MovementLimits {
    /// *V*ₕ in m/s.
    #[must_use]
    pub fn horizontal_speed(&self) -> f64 {
        HORIZONTAL_TOLERANCE_FACTOR * f64::from(self.run_speed)
    }
}

/// What the server does with one `MoveInput` (§8.3).
#[derive(Debug, Clone, PartialEq)]
pub enum MoveVerdict {
    /// Step 2: the epoch is not current; dropped silently.
    StaleEpoch,
    /// Step 3: `seq` is not newer than the last accepted one; dropped.
    OldSeq,
    /// Step 6: accepted. The state now holds the new position; relay it.
    Accepted,
    /// Step 7: rejected; send this `LocationReport(CORRECTED)`. The new epoch has started.
    Corrected(LocationReport),
    /// Step 7 beyond 4 corrections per second: dropped without a report.
    Rejected,
}

/// The server's per-player movement state and check (§8.3).
#[derive(Debug, Clone)]
pub struct MovementValidator {
    limits: MovementLimits,
    epoch: u32,
    last_seq: Option<u32>,
    pose: Pose,
    velocity: Vec3,
    motion: Motion,
    allow_h: f64,
    allow_up: f64,
    allow_down: f64,
    last_time: u64,
    corrections: SlidingWindow,
}

impl MovementValidator {
    /// State for a player who entered the world with `epoch` at `pose`, server time `now`
    /// (µs).
    #[must_use]
    pub fn new(limits: MovementLimits, epoch: u32, pose: Pose, now: u64) -> Self {
        let mut v = Self {
            limits,
            epoch,
            last_seq: None,
            pose,
            velocity: Vec3::ZERO,
            motion: Motion::STANDING,
            allow_h: 0.0,
            allow_up: 0.0,
            allow_down: 0.0,
            last_time: now,
            corrections: SlidingWindow::new(MAX_CORRECTIONS_PER_SECOND, Duration::from_secs(1)),
        };
        v.start_epoch(epoch, pose, now);
        v
    }

    /// Starts a new epoch at `pose` (a teleport or correction): allowances start full, the
    /// velocity is zero and the next `MoveInput` is accepted whatever its `seq`.
    pub fn start_epoch(&mut self, epoch: u32, pose: Pose, now: u64) {
        self.epoch = epoch;
        self.pose = pose;
        self.last_seq = None;
        self.velocity = Vec3::ZERO;
        self.motion = Motion::STANDING;
        let w = self.limits.window.as_secs_f64();
        self.allow_h = self.limits.horizontal_speed() * w;
        self.allow_up = self.limits.up_speed * w;
        self.allow_down = self.limits.down_speed * w;
        self.last_time = now;
    }

    /// Changes the limits (for example a new `RUN_SPEED`).
    pub fn set_limits(&mut self, limits: MovementLimits) {
        self.limits = limits;
    }

    /// The current epoch.
    #[must_use]
    pub fn epoch(&self) -> u32 {
        self.epoch
    }

    /// The last accepted pose.
    #[must_use]
    pub fn pose(&self) -> &Pose {
        &self.pose
    }

    /// The last accepted (clamped) velocity.
    #[must_use]
    pub fn velocity(&self) -> Vec3 {
        self.velocity
    }

    /// The last accepted motion.
    #[must_use]
    pub fn motion(&self) -> Motion {
        self.motion
    }

    fn grow(&mut self, now: u64) {
        let dt = now.saturating_sub(self.last_time) as f64 / 1e6;
        self.last_time = self.last_time.max(now);
        let w = self.limits.window.as_secs_f64();
        let vh = self.limits.horizontal_speed();
        self.allow_h = (self.allow_h + vh * dt).min(vh * w);
        self.allow_up = (self.allow_up + self.limits.up_speed * dt).min(self.limits.up_speed * w);
        self.allow_down =
            (self.allow_down + self.limits.down_speed * dt).min(self.limits.down_speed * w);
    }

    /// Runs steps 2–7 of §8.3 for `input` received at server time `now` (µs). Step 1 (not
    /// InWorld) is the caller's. The caller MAY run further checks of its own after
    /// `Accepted`.
    pub fn check(&mut self, input: &MoveInput, now: u64) -> MoveVerdict {
        if input.epoch != self.epoch {
            return MoveVerdict::StaleEpoch;
        }
        if let Some(last) = self.last_seq
            && !serial_newer(input.seq, last)
        {
            return MoveVerdict::OldSeq;
        }
        self.grow(now);
        let new = input.location;
        let p = self.pose.location;
        let in_extent = |c: f64| c >= self.limits.extent_min && c <= self.limits.extent_max;
        if new.region != p.region || !in_extent(new.x) || !in_extent(new.y) || !new.is_valid() {
            return self.reject(now);
        }
        let dh = p.distance_xy(&new);
        let dz = new.z - p.z;
        let (du, dd) = (dz.max(0.0), (-dz).max(0.0));
        if !(dh <= self.allow_h && du <= self.allow_up && dd <= self.allow_down) {
            return self.reject(now);
        }
        self.allow_h -= dh;
        self.allow_up -= du;
        self.allow_down -= dd;
        self.last_seq = Some(input.seq);
        let anchor = self.pose.anchor.filter(|a| a.owns(&new));
        self.pose = Pose {
            location: new,
            heading: input.heading,
            anchor,
        };
        let vh = self.limits.horizontal_speed() as f32;
        let mut v = input.velocity;
        let h = v.horizontal_length();
        if h > vh && h > 0.0 {
            let k = vh / h;
            v.x *= k;
            v.y *= k;
        }
        v.z = v.z.clamp(
            -(self.limits.down_speed as f32),
            self.limits.up_speed as f32,
        );
        self.velocity = v;
        self.motion = input.motion;
        MoveVerdict::Accepted
    }

    fn reject(&mut self, now: u64) -> MoveVerdict {
        if !self.corrections.try_record(now) {
            return MoveVerdict::Rejected;
        }
        let epoch = self.epoch.wrapping_add(1);
        let pose = self.pose;
        self.start_epoch(epoch, pose, now);
        MoveVerdict::Corrected(LocationReport {
            cause: LocationCause::Corrected,
            request_id: 0,
            epoch,
            server_time: now,
            pose,
        })
    }
}

/// The interest block of an entity (§9.1): its anchor's block if the anchor owns the
/// position, else the landblock containing the position with x and y clamped into
/// [0, 40,800].
#[must_use]
pub fn interest_block(location: &Location, anchor: Option<&Anchor>) -> (u8, u8) {
    if let Some(a) = anchor
        && a.owns(location)
        && a.block_x <= MAX_LANDBLOCK
        && a.block_y <= MAX_LANDBLOCK
    {
        return (a.block_x, a.block_y);
    }
    let index = |c: f64| -> u8 {
        let c = if c.is_finite() {
            c.clamp(0.0, GRID_EXTENT)
        } else {
            0.0
        };
        let i = (c / LANDBLOCK_SIZE).floor().min(f64::from(MAX_LANDBLOCK));
        // 0..=254 after the clamp.
        i as u8
    };
    (index(location.x), index(location.y))
}

/// Chebyshev distance between two blocks.
#[must_use]
pub fn block_distance(a: (u8, u8), b: (u8, u8)) -> u32 {
    u32::from(a.0.abs_diff(b.0)).max(u32::from(a.1.abs_diff(b.1)))
}

/// Interest membership after a re-evaluation (§9.2): an entity enters at distance ≤ *R*,
/// leaves at distance > *R* + 1, and otherwise keeps its membership (hysteresis).
#[must_use]
pub fn in_interest(currently_in: bool, distance: u32, radius: u8) -> bool {
    let r = u32::from(radius);
    if distance <= r {
        true
    } else if distance > r + 1 {
        false
    } else {
        currently_in
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Gait;

    fn input(seq: u32, epoch: u32, x: f64, y: f64, z: f64) -> MoveInput {
        MoveInput {
            seq,
            epoch,
            location: Location::new(1, x, y, z),
            heading: 0.0,
            velocity: Vec3::new(20.0, 0.0, -100.0),
            motion: Motion::new(Gait::Running, false),
        }
    }

    #[test]
    fn serial() {
        assert!(serial_newer(1, 0));
        assert!(serial_newer(0, u32::MAX));
        assert!(!serial_newer(5, 5));
        assert!(!serial_newer(4, 5));
    }

    #[test]
    fn validator() {
        let pose = Pose {
            location: Location::new(1, 1000.0, 1000.0, 10.0),
            heading: 0.0,
            anchor: Some(Anchor::new(6, 6, 3)),
        };
        let mut v = MovementValidator::new(MovementLimits::default(), 1, pose, 0);
        // 7.5 m allowance at start.
        assert_eq!(
            v.check(&input(10, 1, 1007.0, 1000.0, 10.0), 0),
            MoveVerdict::Accepted
        );
        assert_eq!(v.velocity().x, 7.5);
        assert_eq!(v.velocity().z, -60.0);
        assert!(v.pose().anchor.is_some());
        assert_eq!(
            v.check(&input(10, 1, 1007.0, 1000.0, 10.0), 0),
            MoveVerdict::OldSeq
        );
        assert_eq!(
            v.check(&input(11, 0, 1007.0, 1000.0, 10.0), 0),
            MoveVerdict::StaleEpoch
        );
        // Half a second later: 0.5 + 3.75 m available.
        assert_eq!(
            v.check(&input(11, 1, 1011.0, 1000.0, 10.0), 500_000),
            MoveVerdict::Accepted
        );
        match v.check(&input(12, 1, 1020.0, 1000.0, 10.0), 500_000) {
            MoveVerdict::Corrected(r) => {
                assert_eq!(r.epoch, 2);
                assert_eq!(r.pose.location.x, 1011.0);
                assert_eq!(r.request_id, 0);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(v.epoch(), 2);
        // Wrong region and leaving the extent are rejected.
        let mut bad = input(1, 2, 1011.0, 1000.0, 10.0);
        bad.location.region = 2;
        assert!(matches!(v.check(&bad, 600_000), MoveVerdict::Corrected(_)));
        assert!(matches!(
            v.check(&input(1, 3, -1281.0, 1000.0, 10.0), 600_000),
            MoveVerdict::Corrected(_)
        ));
        // Four corrections per second at most.
        assert!(matches!(
            v.check(&input(1, 4, 5000.0, 1000.0, 10.0), 600_000),
            MoveVerdict::Corrected(_)
        ));
        assert_eq!(
            v.check(&input(1, 5, 5000.0, 1000.0, 10.0), 600_000),
            MoveVerdict::Rejected
        );
        assert_eq!(v.epoch(), 5);
        // Leaving the anchor's square drops the anchor.
        assert!(matches!(
            v.check(&input(1, 5, 1011.0, 1000.0, 3.0), 2_000_000),
            MoveVerdict::Accepted
        ));
    }

    #[test]
    fn interest() {
        let l = Location::new(1, 20_250.5, 22_500.25, 0.0);
        assert_eq!(interest_block(&l, None), (126, 140));
        assert_eq!(
            interest_block(&l, Some(&Anchor::new(127, 140, 1))),
            (127, 140)
        );
        assert_eq!(
            interest_block(&Location::new(1, -50.0, 99_999.0, 0.0), None),
            (0, 254)
        );
        assert_eq!(block_distance((1, 1), (3, 0)), 2);
        assert!(in_interest(false, 2, 2));
        assert!(!in_interest(false, 3, 2));
        assert!(in_interest(true, 3, 2));
        assert!(!in_interest(true, 4, 2));
    }

    #[test]
    fn schedule() {
        let mut s = MoveScheduler::new();
        let run = Motion::new(Gait::Running, false);
        let stand = Motion::STANDING;
        let sends: Vec<bool> = (0..8).map(|t| s.on_tick(t, true, run, false)).collect();
        assert_eq!(sends, [true, false, true, false, true, false, true, false]);
        // Stop: send now and the next three ticks, then a 1 Hz heartbeat.
        assert!(s.on_tick(8, false, stand, false));
        assert!(s.on_tick(9, false, stand, false));
        assert!(s.on_tick(10, false, stand, false));
        assert!(s.on_tick(11, false, stand, false));
        assert!(!s.on_tick(12, false, stand, false));
        assert!(!s.on_tick(40, false, stand, false));
        assert!(s.on_tick(41, false, stand, false));
        assert!(s.on_tick(42, false, stand, true));
    }
}
